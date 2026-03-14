pub mod config_cmd;
pub mod ig;
pub mod sp;
pub mod tk;
pub mod tw;
pub mod update;
pub mod yt;

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use yt_dlp::model::selector::{
    AudioCodecPreference, AudioQuality, VideoCodecPreference, VideoQuality,
};
use yt_dlp::model::types::playlist::PlaylistEntry;
use yt_dlp::Downloader;

use crate::cli::{CommonArgs, SpotifyArgs};
use crate::config::{self, Config};
use crate::{downloader, sandbox, tui};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Platform {
    YouTube,
    TikTok,
    Instagram,
    Twitter,
    Spotify,
}

#[derive(Debug, Clone)]
struct CommonRequest {
    url: Option<String>,
    search: Option<String>,
    quality: String,
    format: String,
    output_dir: PathBuf,
    audio_only: bool,
    video_only: bool,
    yes: bool,
}

#[derive(Debug, Clone)]
struct SpotifyRequest {
    url: String,
    format: String,
    output_dir: PathBuf,
    yes: bool,
}

impl Platform {
    fn label(self) -> &'static str {
        match self {
            Platform::YouTube => "YouTube",
            Platform::TikTok => "TikTok",
            Platform::Instagram => "Instagram",
            Platform::Twitter => "Twitter/X",
            Platform::Spotify => "Spotify",
        }
    }

    fn quality_override<'a>(self, cfg: &'a Config) -> Option<&'a str> {
        match self {
            Platform::YouTube => Some(cfg.platform_quality.youtube.as_str()),
            Platform::TikTok => Some(cfg.platform_quality.tiktok.as_str()),
            Platform::Instagram => Some(cfg.platform_quality.instagram.as_str()),
            Platform::Twitter => Some(cfg.platform_quality.twitter.as_str()),
            Platform::Spotify => Some(cfg.platform_quality.spotify.as_str()),
        }
    }

    fn supports_search(self) -> bool {
        matches!(self, Platform::YouTube)
    }
}

pub(crate) async fn run_common_platform(platform: Platform, args: CommonArgs) -> Result<()> {
    let Some(cfg) = load_config_or_create()? else {
        return Ok(());
    };

    sandbox::ensure_installed(&cfg).await?;
    let request = normalize_common_args(platform, args, &cfg)?;

    fs::create_dir_all(&request.output_dir).with_context(|| {
        format!(
            "Failed to create output directory at {}",
            request.output_dir.display()
        )
    })?;

    let downloader = downloader::build(&cfg).await?;

    let url = if let Some(url) = request.url.clone() {
        tui::print_header(platform.label(), "Downloading");
        url
    } else {
        let query = request
            .search
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("Search query was not provided"))?;
        match search_for_url(platform, &downloader, &cfg, query).await? {
            Some(url) => {
                tui::print_header(platform.label(), "Downloading");
                url
            }
            None => return Ok(()),
        }
    };

    let video = fetch_video(&downloader, &url).await?;
    tui::print_metadata(&video);

    if cfg.confirm_before_download && !request.yes && !tui::confirm_download(&video.title)? {
        tui::print_info("Download cancelled.");
        return Ok(());
    }

    let filename = format!(
        "{}.{}",
        sanitise_filename(&video.title),
        request.format.as_str()
    );
    let output_path = request.output_dir.join(&filename);
    let saved_path =
        execute_common_download(&downloader, &video, &output_path, &filename, &request).await?;

    print_download_summary(&saved_path)?;
    Ok(())
}

pub(crate) async fn run_spotify(args: SpotifyArgs) -> Result<()> {
    let Some(cfg) = load_config_or_create()? else {
        return Ok(());
    };

    sandbox::ensure_installed(&cfg).await?;
    let request = normalize_spotify_args(args, &cfg)?;

    fs::create_dir_all(&request.output_dir).with_context(|| {
        format!(
            "Failed to create output directory at {}",
            request.output_dir.display()
        )
    })?;

    let downloader = downloader::build(&cfg).await?;

    tui::print_header(Platform::Spotify.label(), "Downloading");
    let video = fetch_video(&downloader, &request.url).await?;
    tui::print_spotify_metadata(&video);

    if cfg.confirm_before_download && !request.yes && !tui::confirm_download(&video.title)? {
        tui::print_info("Download cancelled.");
        return Ok(());
    }

    let filename = format!(
        "{}.{}",
        sanitise_filename(&video.title),
        request.format.as_str()
    );
    let output_path = request.output_dir.join(&filename);
    let progress = tui::progress_bar(&filename);
    let progress_for_callback = progress.clone();

    // yt-dlp 2.6.0 exposes public progress callbacks for stream downloads via DownloadBuilder::with_progress(f64).
    let result = downloader
        .download(&video, output_path.clone())
        .audio_quality(AudioQuality::Best)
        .audio_codec(audio_codec_for_format(&request.format))
        .with_progress(move |fraction| tui::update_progress_bar(&progress_for_callback, fraction))
        .execute_audio_stream()
        .await;

    let saved_path = match result {
        Ok(path) => {
            tui::clear_progress(&progress);
            path
        }
        Err(err) => {
            tui::clear_progress(&progress);
            return Err(err).context(
                "Tip: Spotify downloads may require authentication. See vdl config for cookie options.",
            );
        }
    };

    print_download_summary(&saved_path)?;
    Ok(())
}

pub(crate) fn load_config_or_create() -> Result<Option<Config>> {
    if Config::ensure_exists()? {
        let path = config::config_path()?;
        tui::print_first_run(&display_path(&path));
        return Ok(None);
    }

    Ok(Some(Config::load()?))
}

fn normalize_common_args(
    platform: Platform,
    args: CommonArgs,
    cfg: &Config,
) -> Result<CommonRequest> {
    let mut audio_only = args.audio_only;
    let video_only = args.video_only;
    let url = normalize_optional_text(args.url);
    let mut search = normalize_optional_text(args.search);

    let mut format = args.format.unwrap_or_else(|| {
        if audio_only {
            "mp3".to_string()
        } else {
            cfg.default_format.clone()
        }
    });

    ensure_valid_common_format(&format)?;
    if is_audio_format(&format) {
        audio_only = true;
    }

    if audio_only && video_only {
        bail!("--audio-only and --video-only are mutually exclusive.");
    }

    if url.is_some() && search.is_some() {
        bail!("--url and --search are mutually exclusive.");
    }

    if url.is_none() && search.is_none() {
        let query = tui::prompt_input("Enter search query")?;
        if query.trim().is_empty() {
            bail!("Search query cannot be empty.");
        }
        search = Some(query);
    }

    let quality = args
        .quality
        .or_else(|| platform.quality_override(cfg).map(str::to_string))
        .unwrap_or_else(|| cfg.default_video_quality.clone());
    ensure_valid_quality(&quality)?;

    let output_dir = resolve_output_dir(args.output, cfg.download_path_expanded())?;

    if video_only && is_audio_format(&format) {
        bail!("Audio formats cannot be used with --video-only.");
    }

    Ok(CommonRequest {
        url,
        search,
        quality,
        format: std::mem::take(&mut format),
        output_dir,
        audio_only,
        video_only,
        yes: args.yes,
    })
}

fn normalize_spotify_args(args: SpotifyArgs, cfg: &Config) -> Result<SpotifyRequest> {
    let url = match normalize_optional_text(args.url) {
        Some(url) => url,
        _ => tui::prompt_input("Enter Spotify URL")?,
    };

    let format = args.format.unwrap_or_else(|| "mp3".to_string());
    ensure_valid_spotify_format(&format)?;
    let output_dir = resolve_output_dir(args.output, cfg.download_path_expanded())?;

    Ok(SpotifyRequest {
        url,
        format,
        output_dir,
        yes: args.yes,
    })
}

async fn search_for_url(
    platform: Platform,
    downloader: &Downloader,
    cfg: &Config,
    query: &str,
) -> Result<Option<String>> {
    tui::print_header(platform.label(), "Searching");
    let spinner = tui::spinner(&format!(
        "Searching {} for \"{}\"...",
        platform.label(),
        query
    ));

    if !platform.supports_search() {
        tui::clear_progress(&spinner);
        bail!("Search is only supported for YouTube. Please provide a URL with --url.");
    }

    let playlist = downloader
        .youtube_extractor()
        .search(query, cfg.search_results_count)
        .await
        .context("Failed to search YouTube")?;
    tui::spinner_ok(
        &spinner,
        &format!("Found {} results", playlist.entries.len()),
    );

    let results = playlist
        .entries
        .iter()
        .map(map_search_result)
        .collect::<Vec<_>>();

    if results.is_empty() {
        tui::print_info("No results found.");
        return Ok(None);
    }

    let Some(index) = tui::select_search_result(&results)? else {
        return Ok(None);
    };

    Ok(Some(format!(
        "https://www.youtube.com/watch?v={}",
        results[index].id
    )))
}

async fn fetch_video(downloader: &Downloader, url: &str) -> Result<yt_dlp::model::Video> {
    let spinner = tui::spinner("Fetching video info...");
    let video = downloader
        .fetch_video_infos(url)
        .await
        .with_context(|| format!("Failed to fetch video info for {url}"))?;

    tui::spinner_ok(&spinner, "Fetched video info");
    Ok(video)
}

async fn execute_common_download(
    downloader: &Downloader,
    video: &yt_dlp::model::Video,
    output_path: &Path,
    filename: &str,
    request: &CommonRequest,
) -> Result<PathBuf> {
    let progress = tui::progress_bar(filename);

    // yt-dlp 2.6.0 exposes public progress callbacks for combined and stream downloads
    // through DownloadBuilder::with_progress(f64), not the byte-based signatures used in the spec.
    let result = if request.audio_only {
        execute_audio_download(downloader, video, output_path, request, &progress).await
    } else if request.video_only {
        execute_video_only_download(downloader, video, output_path, request, &progress).await
    } else {
        execute_full_download(downloader, video, output_path, request, &progress).await
    };

    match result {
        Ok(path) => {
            tui::clear_progress(&progress);
            Ok(path)
        }
        Err(err) => {
            tui::clear_progress(&progress);
            Err(err)
        }
    }
}

async fn execute_full_download(
    downloader: &Downloader,
    video: &yt_dlp::model::Video,
    output_path: &Path,
    request: &CommonRequest,
    progress: &indicatif::ProgressBar,
) -> Result<PathBuf> {
    let progress_for_callback = progress.clone();

    downloader
        .download(video, output_path.to_path_buf())
        .video_quality(map_quality(&request.quality))
        .video_codec(VideoCodecPreference::AVC1)
        .audio_quality(AudioQuality::Best)
        .with_progress(move |fraction| tui::update_progress_bar(&progress_for_callback, fraction))
        .execute()
        .await
        .context("Failed to download video")
}

async fn execute_audio_download(
    downloader: &Downloader,
    video: &yt_dlp::model::Video,
    output_path: &Path,
    request: &CommonRequest,
    progress: &indicatif::ProgressBar,
) -> Result<PathBuf> {
    let progress_for_callback = progress.clone();

    downloader
        .download(video, output_path.to_path_buf())
        .audio_quality(AudioQuality::Best)
        .audio_codec(audio_codec_for_format(&request.format))
        .with_progress(move |fraction| tui::update_progress_bar(&progress_for_callback, fraction))
        .execute_audio_stream()
        .await
        .context("Failed to download audio stream")
}

async fn execute_video_only_download(
    downloader: &Downloader,
    video: &yt_dlp::model::Video,
    output_path: &Path,
    request: &CommonRequest,
    progress: &indicatif::ProgressBar,
) -> Result<PathBuf> {
    let progress_for_callback = progress.clone();

    downloader
        .download(video, output_path.to_path_buf())
        .video_quality(map_quality(&request.quality))
        .video_codec(VideoCodecPreference::AVC1)
        .with_progress(move |fraction| tui::update_progress_bar(&progress_for_callback, fraction))
        .execute_video_stream()
        .await
        .context("Failed to download video stream")
}

fn print_download_summary(saved_path: &Path) -> Result<()> {
    let metadata = fs::metadata(saved_path).with_context(|| {
        format!(
            "Failed to read output file metadata at {}",
            saved_path.display()
        )
    })?;

    tui::print_success(&format!("Saved to: {}", saved_path.display()));
    tui::print_success(&format!("File size: {}", format_size(metadata.len())));
    Ok(())
}

fn map_search_result(entry: &PlaylistEntry) -> tui::SearchResult {
    tui::SearchResult {
        title: entry.title.clone(),
        uploader: entry
            .uploader
            .clone()
            .unwrap_or_else(|| "Unknown".to_string()),
        duration: format_playlist_duration(entry.duration),
        id: entry.id.clone(),
    }
}

fn resolve_output_dir(override_dir: Option<PathBuf>, fallback: PathBuf) -> Result<PathBuf> {
    let raw = override_dir.map(expand_user_path).unwrap_or(fallback);
    if raw.is_absolute() {
        Ok(raw)
    } else {
        Ok(std::env::current_dir()
            .context("Failed to resolve current working directory")?
            .join(raw))
    }
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn expand_user_path(path: PathBuf) -> PathBuf {
    let Some(home) = dirs::home_dir() else {
        return path;
    };

    let rendered = path.to_string_lossy();
    if rendered == "~" {
        return home;
    }

    if let Some(stripped) = rendered
        .strip_prefix("~/")
        .or_else(|| rendered.strip_prefix("~\\"))
    {
        return home.join(stripped);
    }

    path
}

fn ensure_valid_quality(value: &str) -> Result<()> {
    if matches!(value, "best" | "1080" | "720" | "480" | "360" | "worst") {
        Ok(())
    } else {
        bail!("Invalid quality \"{value}\". Expected one of: best, 1080, 720, 480, 360, worst.");
    }
}

fn ensure_valid_common_format(value: &str) -> Result<()> {
    if matches!(value, "mp4" | "mkv" | "webm" | "mp3" | "m4a" | "opus") {
        Ok(())
    } else {
        bail!("Invalid format \"{value}\".");
    }
}

fn ensure_valid_spotify_format(value: &str) -> Result<()> {
    if matches!(value, "mp3" | "m4a" | "opus") {
        Ok(())
    } else {
        bail!("Spotify only supports mp3, m4a, or opus output.");
    }
}

fn is_audio_format(value: &str) -> bool {
    matches!(value, "mp3" | "m4a" | "opus")
}

fn audio_codec_for_format(format: &str) -> AudioCodecPreference {
    match format {
        "mp3" => AudioCodecPreference::MP3,
        "m4a" => AudioCodecPreference::AAC,
        "opus" => AudioCodecPreference::Opus,
        _ => AudioCodecPreference::Any,
    }
}

fn map_quality(value: &str) -> VideoQuality {
    match value {
        "best" => VideoQuality::Best,
        "1080" => VideoQuality::High,
        "720" => VideoQuality::Medium,
        "480" => VideoQuality::Low,
        "360" => VideoQuality::CustomHeight(360),
        "worst" => VideoQuality::Worst,
        _ => VideoQuality::Best,
    }
}

fn sanitise_filename(title: &str) -> String {
    let mut sanitized = String::new();
    let mut last_was_separator = false;

    for ch in title.chars() {
        let normalized = if ch.is_alphanumeric() || ch == '-' {
            ch
        } else if ch == '_' || ch.is_whitespace() {
            '_'
        } else {
            '_'
        };

        if normalized == '_' {
            if !last_was_separator {
                sanitized.push('_');
                last_was_separator = true;
            }
        } else {
            sanitized.push(normalized);
            last_was_separator = false;
        }

        if sanitized.chars().count() >= 80 {
            break;
        }
    }

    let trimmed = sanitized.trim_matches('_').to_string();
    if trimmed.is_empty() {
        "download".to_string()
    } else {
        trimmed
    }
}

fn format_size(bytes: u64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut index = 0usize;

    while size >= 1024.0 && index < units.len() - 1 {
        size /= 1024.0;
        index += 1;
    }

    if index == 0 {
        format!("{bytes} {}", units[index])
    } else if size >= 10.0 {
        format!("{size:.0} {}", units[index])
    } else {
        format!("{size:.1} {}", units[index])
    }
}

fn format_playlist_duration(seconds: Option<f64>) -> String {
    let Some(seconds) = seconds else {
        return "Unknown".to_string();
    };

    let total_seconds = seconds.max(0.0).round() as i64;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    format!("{hours}:{minutes:02}:{seconds:02}")
}

pub(crate) fn display_path(path: &Path) -> String {
    if let Some(home) = dirs::home_dir() {
        if let Ok(stripped) = path.strip_prefix(&home) {
            if stripped.as_os_str().is_empty() {
                return "~".to_string();
            }

            return format!("~/{}", stripped.display());
        }
    }

    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_quality_matches_expected_variants() {
        assert_eq!(map_quality("best"), VideoQuality::Best);
        assert_eq!(map_quality("1080"), VideoQuality::High);
        assert_eq!(map_quality("720"), VideoQuality::Medium);
        assert_eq!(map_quality("480"), VideoQuality::Low);
        assert_eq!(map_quality("360"), VideoQuality::CustomHeight(360));
        assert_eq!(map_quality("worst"), VideoQuality::Worst);
    }

    #[test]
    fn sanitise_filename_replaces_invalid_chars_and_trims() {
        assert_eq!(
            sanitise_filename("  Rust: async/await? *guide*  "),
            "Rust_async_await_guide"
        );
        assert_eq!(sanitise_filename("___"), "download");
    }

    #[test]
    fn format_size_uses_human_units() {
        assert_eq!(format_size(999), "999 B");
        assert_eq!(format_size(1_536), "1.5 KB");
    }

    #[test]
    fn resolve_output_dir_expands_tilde_overrides() {
        let home = dirs::home_dir().expect("home dir should exist");
        let output = resolve_output_dir(Some(PathBuf::from("~/Downloads/vdl")), PathBuf::new())
            .expect("tilde output path should resolve");

        assert_eq!(output, home.join("Downloads/vdl"));
    }

    #[test]
    fn display_path_uses_tilde_for_home_relative_paths() {
        let home = dirs::home_dir().expect("home dir should exist");
        let path = home.join(".config/vdl/config.yaml");

        assert_eq!(display_path(&path), "~/.config/vdl/config.yaml");
    }
}

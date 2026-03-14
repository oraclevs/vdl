/// Shows the current config file path and colourised YAML contents.
pub mod config_cmd;
/// Handles Instagram downloads through the shared platform flow.
pub mod ig;
/// Handles Spotify audio downloads.
pub mod sp;
/// Handles TikTok downloads through the shared platform flow.
pub mod tk;
/// Handles Twitter/X downloads through the shared platform flow.
pub mod tw;
/// Updates sandboxed helper binaries.
pub mod update;
/// Handles YouTube downloads and interactive search.
pub mod yt;

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde_json::{json, Map, Value};
use yt_dlp::error::Error as YtDlpError;
use yt_dlp::events::{DownloadEvent, PostProcessOperation};
use yt_dlp::model::selector::{
    AudioCodecPreference, AudioQuality, VideoCodecPreference, VideoQuality,
};
use yt_dlp::model::types::playlist::PlaylistEntry;
use yt_dlp::model::Video;
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

    fn quality_override(self, cfg: &Config) -> Option<&str> {
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

    fn auth_hint(self) -> Option<&'static str> {
        match self {
            Platform::YouTube => Some(
                "Tip: Some YouTube videos require authentication. Set cookies_from_browser or cookies_file in ~/.config/vdl/config.yaml and run `vdl update` if extractor support is outdated.",
            ),
            Platform::Instagram => Some(
                "Tip: Some Instagram posts require login. Set cookies_from_browser or cookies_file in ~/.config/vdl/config.yaml.",
            ),
            Platform::Spotify => Some(
                "Tip: Spotify downloads may require authentication. Set cookies_from_browser or cookies_file in ~/.config/vdl/config.yaml.",
            ),
            Platform::TikTok | Platform::Twitter => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DownloadUiMode {
    Combined,
    AudioOnly,
    VideoOnly,
}

impl DownloadUiMode {
    fn initial_message(self) -> &'static str {
        match self {
            Self::Combined => "3/4 Preparing media streams...",
            Self::AudioOnly => "3/4 Preparing audio download...",
            Self::VideoOnly => "3/4 Preparing video download...",
        }
    }

    fn started_message(self, started_downloads: usize) -> &'static str {
        match self {
            Self::Combined if started_downloads <= 1 => "3/4 Downloading video stream...",
            Self::Combined => "3/4 Downloading audio stream...",
            Self::AudioOnly => "3/4 Downloading audio stream...",
            Self::VideoOnly => "3/4 Downloading video stream...",
        }
    }

    fn completed_message(self, completed_downloads: usize) -> &'static str {
        match self {
            Self::Combined if completed_downloads == 1 => "3/4 Waiting for the audio stream...",
            Self::Combined => "3/4 Finalising media...",
            Self::AudioOnly => "3/4 Finalising audio file...",
            Self::VideoOnly => "3/4 Finalising video file...",
        }
    }

    fn post_process_message(self, operation: &PostProcessOperation) -> &'static str {
        match (self, operation) {
            (Self::Combined, PostProcessOperation::CombineStreams { .. }) => {
                "3/4 Merging video and audio..."
            }
            (_, PostProcessOperation::SplitChapters { .. }) => "3/4 Splitting chapters...",
            _ => "3/4 Applying post-processing...",
        }
    }
}

struct DownloadUiSession {
    stop_tx: Option<tokio::sync::oneshot::Sender<()>>,
    handle: tokio::task::JoinHandle<()>,
}

impl DownloadUiSession {
    fn start(
        downloader: &Downloader,
        progress: &indicatif::ProgressBar,
        mode: DownloadUiMode,
    ) -> Self {
        let mut receiver = downloader.subscribe_events();
        let progress = progress.clone();
        let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel();

        tui::set_progress_message(&progress, mode.initial_message());

        let handle = tokio::spawn(async move {
            let mut started_downloads = 0usize;
            let mut completed_downloads = 0usize;

            loop {
                tokio::select! {
                    _ = &mut stop_rx => break,
                    event = receiver.recv() => match event {
                        Ok(event) => match &*event {
                            DownloadEvent::DownloadQueued { .. } => {
                                tui::set_progress_message(&progress, mode.initial_message());
                            }
                            DownloadEvent::DownloadStarted { .. } => {
                                started_downloads += 1;
                                tui::set_progress_message(
                                    &progress,
                                    mode.started_message(started_downloads),
                                );
                            }
                            DownloadEvent::DownloadCompleted { .. } => {
                                completed_downloads += 1;
                                match mode {
                                    DownloadUiMode::Combined => {
                                        if completed_downloads == 1 {
                                            tui::advance_progress_bar(&progress, 0.55);
                                        } else {
                                            tui::advance_progress_bar(&progress, 0.9);
                                        }
                                    }
                                    DownloadUiMode::AudioOnly | DownloadUiMode::VideoOnly => {
                                        tui::advance_progress_bar(&progress, 0.95);
                                    }
                                }
                                tui::set_progress_message(
                                    &progress,
                                    mode.completed_message(completed_downloads),
                                );
                            }
                            DownloadEvent::PostProcessStarted { operation, .. } => {
                                tui::advance_progress_bar(&progress, 0.95);
                                tui::set_progress_message(
                                    &progress,
                                    mode.post_process_message(operation),
                                );
                            }
                            DownloadEvent::MetadataApplied { .. } => {
                                tui::advance_progress_bar(&progress, 0.98);
                                tui::set_progress_message(
                                    &progress,
                                    "3/4 Writing metadata...",
                                );
                            }
                            DownloadEvent::ChaptersEmbedded { .. } => {
                                tui::advance_progress_bar(&progress, 0.98);
                                tui::set_progress_message(
                                    &progress,
                                    "3/4 Embedding chapters...",
                                );
                            }
                            DownloadEvent::PostProcessCompleted { .. } => {
                                tui::advance_progress_bar(&progress, 0.99);
                                tui::set_progress_message(
                                    &progress,
                                    "3/4 Post-processing complete...",
                                );
                            }
                            _ => {}
                        },
                        Err(_) => break,
                    }
                }
            }
        });

        Self {
            stop_tx: Some(stop_tx),
            handle,
        }
    }

    async fn stop(mut self) {
        if let Some(stop_tx) = self.stop_tx.take() {
            let _ = stop_tx.send(());
        }
        let _ = self.handle.await;
    }
}

pub(crate) async fn run_common_platform(platform: Platform, args: CommonArgs) -> Result<()> {
    let Some(cfg) = load_config_or_create()? else {
        return Ok(());
    };

    let request = normalize_common_args(platform, args, &cfg)?;
    tui::print_header(
        platform.label(),
        if request.url.is_some() {
            "Downloading"
        } else {
            "Searching"
        },
    );
    let downloader = prepare_download_environment(&cfg, &request.output_dir).await?;

    let url = if let Some(url) = request.url.clone() {
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

    let video =
        fetch_video(&downloader, &cfg, &url)
            .await
            .map_err(|err| match platform.auth_hint() {
                Some(hint) => err.context(hint),
                None => err,
            })?;
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
        execute_common_download(&cfg, &downloader, &video, &output_path, &filename, &request)
            .await?;

    print_download_summary(&saved_path)?;
    Ok(())
}

pub(crate) async fn run_spotify(args: SpotifyArgs) -> Result<()> {
    let Some(cfg) = load_config_or_create()? else {
        return Ok(());
    };

    let request = normalize_spotify_args(args, &cfg)?;
    tui::print_header(Platform::Spotify.label(), "Downloading");
    let downloader = prepare_download_environment(&cfg, &request.output_dir).await?;
    let video = fetch_video(&downloader, &cfg, &request.url)
        .await
        .map_err(|err| match Platform::Spotify.auth_hint() {
            Some(hint) => err.context(hint),
            None => err,
        })?;
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
        .with_progress(move |fraction| tui::advance_progress_bar(&progress_for_callback, fraction))
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
                "Tip: Spotify downloads may require authentication. Set cookies_from_browser or cookies_file in ~/.config/vdl/config.yaml.",
            );
        }
    };

    print_download_summary(&saved_path)?;
    Ok(())
}

pub(crate) fn load_config_or_create() -> Result<Option<Config>> {
    if Config::ensure_exists()? {
        let path = config::config_path().context("Failed to resolve vdl config path")?;
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

async fn fetch_video(downloader: &Downloader, cfg: &Config, url: &str) -> Result<Video> {
    let spinner = tui::spinner("2/4 Fetching video metadata...");
    let video = match downloader.fetch_video_infos(url).await {
        Ok(video) => video,
        // yt-dlp 2.6.0's Video model requires fields that some extractors (notably TikTok)
        // return as missing or null. Fall back to raw `yt-dlp -J` output and normalize them.
        Err(YtDlpError::Json { .. }) => fetch_video_via_raw_json(cfg, url)
            .await
            .with_context(|| format!("Failed to fetch video info for {url}"))?,
        Err(err) => {
            return Err(err).with_context(|| format!("Failed to fetch video info for {url}"))
        }
    };

    tui::spinner_ok(&spinner, "2/4 Video metadata ready");
    Ok(video)
}

async fn fetch_video_via_raw_json(cfg: &Config, url: &str) -> Result<Video> {
    let output = tokio::process::Command::new(sandbox::ytdlp_path(cfg))
        .arg("-J")
        .arg(url)
        .output()
        .await
        .with_context(|| format!("Failed to execute sandboxed yt-dlp for {url}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        bail!(
            "Sandboxed yt-dlp metadata fetch failed for {url}: {}",
            if stderr.is_empty() {
                "Unknown error"
            } else {
                &stderr
            }
        );
    }

    let mut value: Value = serde_json::from_slice(&output.stdout)
        .with_context(|| format!("Failed to parse raw yt-dlp metadata for {url}"))?;

    normalize_video_json(&mut value);

    let mut video: Video = serde_json::from_value(value)
        .with_context(|| format!("Failed to deserialize normalized metadata for {url}"))?;

    for format in &mut video.formats {
        format.video_id = Some(video.id.clone());
    }

    Ok(video)
}

fn normalize_video_json(value: &mut Value) {
    let Some(object) = value.as_object_mut() else {
        return;
    };

    set_default_scalar(object, "age_limit", json!(0));
    set_default_scalar(object, "live_status", json!("not_live"));
    set_default_scalar(object, "playable_in_embed", json!(true));
    set_default_array(object, "formats");
    set_default_array(object, "thumbnails");
    set_default_object(object, "automatic_captions");
    set_default_object(object, "subtitles");
    set_default_array(object, "tags");
    set_default_array(object, "categories");
}

fn set_default_scalar(object: &mut Map<String, Value>, key: &str, default: Value) {
    if matches!(object.get(key), None | Some(Value::Null)) {
        object.insert(key.to_string(), default);
    }
}

fn set_default_array(object: &mut Map<String, Value>, key: &str) {
    if matches!(object.get(key), None | Some(Value::Null)) {
        object.insert(key.to_string(), Value::Array(Vec::new()));
    }
}

fn set_default_object(object: &mut Map<String, Value>, key: &str) {
    if matches!(object.get(key), None | Some(Value::Null)) {
        object.insert(key.to_string(), Value::Object(Map::new()));
    }
}

async fn execute_common_download(
    cfg: &Config,
    downloader: &Downloader,
    video: &yt_dlp::model::Video,
    output_path: &Path,
    filename: &str,
    request: &CommonRequest,
) -> Result<PathBuf> {
    let mode = if request.audio_only {
        DownloadUiMode::AudioOnly
    } else if request.video_only {
        DownloadUiMode::VideoOnly
    } else {
        DownloadUiMode::Combined
    };
    let progress = tui::progress_bar(filename);
    let ui_session = DownloadUiSession::start(downloader, &progress, mode);
    let temp_dir = cfg.download_path_expanded();

    // yt-dlp 2.6.0 exposes public progress callbacks for combined and stream downloads
    // through DownloadBuilder::with_progress(f64), not the byte-based signatures used in the spec.
    let result = if request.audio_only {
        execute_audio_download(downloader, video, output_path, request, &progress).await
    } else if request.video_only {
        execute_video_only_download(downloader, video, output_path, request, &progress).await
    } else {
        execute_full_download(
            cfg,
            downloader,
            video,
            output_path,
            filename,
            request,
            &progress,
        )
        .await
    };
    ui_session.stop().await;

    match result {
        Ok(path) => {
            tui::clear_progress(&progress);
            if let Err(err) = cleanup_download_artifacts(&temp_dir).await {
                tui::print_warning(&format!(
                    "Download completed, but failed to clean temporary files in {}: {err}",
                    temp_dir.display()
                ));
            }
            Ok(path)
        }
        Err(err) => {
            tui::clear_progress(&progress);
            if let Err(cleanup_err) = cleanup_download_artifacts(&temp_dir).await {
                tui::print_warning(&format!(
                    "Failed to clean temporary files in {} after download error: {cleanup_err}",
                    temp_dir.display()
                ));
            }
            Err(err)
        }
    }
}

async fn execute_full_download(
    cfg: &Config,
    downloader: &Downloader,
    video: &yt_dlp::model::Video,
    output_path: &Path,
    filename: &str,
    request: &CommonRequest,
    progress: &indicatif::ProgressBar,
) -> Result<PathBuf> {
    let progress_for_callback = progress.clone();
    let result = downloader
        .download(video, output_path.to_path_buf())
        .video_quality(map_quality(&request.quality))
        .video_codec(VideoCodecPreference::AVC1)
        .audio_quality(AudioQuality::Best)
        .with_progress(move |fraction| tui::advance_progress_bar(&progress_for_callback, fraction))
        .execute()
        .await;

    match result {
        Ok(path) => Ok(path),
        Err(YtDlpError::FormatNotAvailable { .. }) => {
            tui::clear_progress(progress);
            execute_combined_video_download(cfg, downloader, video, output_path, filename).await
        }
        Err(err) => Err(err).context("Failed to download video"),
    }
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
        .with_progress(move |fraction| tui::advance_progress_bar(&progress_for_callback, fraction))
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
        .with_progress(move |fraction| tui::advance_progress_bar(&progress_for_callback, fraction))
        .execute_video_stream()
        .await
        .context("Failed to download video stream")
}

async fn execute_combined_video_download(
    cfg: &Config,
    downloader: &Downloader,
    video: &yt_dlp::model::Video,
    output_path: &Path,
    filename: &str,
) -> Result<PathBuf> {
    let spinner = tui::spinner(&format!("Downloading {filename}..."));
    let format = video
        .best_audio_video_format()
        .context("No combined video format available")?;

    match downloader
        .download_format_to_path(format, output_path.to_path_buf())
        .await
    {
        Ok(path) => {
            tui::spinner_ok(&spinner, &format!("Downloaded {filename}"));
            Ok(path)
        }
        Err(err) => match execute_binary_video_download(cfg, video, output_path).await {
            Ok(path) => {
                tui::spinner_ok(&spinner, &format!("Downloaded {filename}"));
                Ok(path)
            }
            Err(binary_err) => {
                tui::spinner_err(&spinner, "Failed to download combined video format");
                Err(binary_err).context(format!(
                    "Failed to download combined video format after crate fallback error: {err}"
                ))
            }
        },
    }
}

async fn execute_binary_video_download(
    cfg: &Config,
    video: &yt_dlp::model::Video,
    output_path: &Path,
) -> Result<PathBuf> {
    let url = video
        .webpage_url
        .as_deref()
        .context("Video metadata did not contain a source URL for yt-dlp fallback")?;

    let output = tokio::process::Command::new(sandbox::ytdlp_path(cfg))
        .arg("-o")
        .arg(output_path.as_os_str())
        .arg(url)
        .output()
        .await
        .with_context(|| format!("Failed to execute sandboxed yt-dlp download for {url}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        bail!(
            "{}",
            if stderr.is_empty() {
                format!("Sandboxed yt-dlp download failed for {url}")
            } else {
                stderr
            }
        );
    }

    Ok(output_path.to_path_buf())
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

async fn prepare_download_environment(cfg: &Config, output_dir: &Path) -> Result<Downloader> {
    let spinner = tui::spinner("1/4 Preparing download environment...");
    let result = async {
        sandbox::ensure_installed(cfg).await?;
        fs::create_dir_all(output_dir).with_context(|| {
            format!(
                "Failed to create output directory at {}",
                output_dir.display()
            )
        })?;
        downloader::build(cfg).await
    }
    .await;

    match result {
        Ok(downloader) => {
            tui::spinner_ok(&spinner, "1/4 Download environment ready");
            Ok(downloader)
        }
        Err(err) => {
            tui::spinner_err(&spinner, "1/4 Failed to prepare download environment");
            Err(err)
        }
    }
}

async fn cleanup_managed_temp_files(dir: &Path) -> Result<usize> {
    let mut removed = 0usize;
    let mut entries = match tokio::fs::read_dir(dir).await {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(err) => {
            return Err(err)
                .with_context(|| format!("Failed to read temp directory {}", dir.display()))
        }
    };

    while let Some(entry) = entries
        .next_entry()
        .await
        .with_context(|| format!("Failed to read temp directory entry in {}", dir.display()))?
    {
        let file_type = entry
            .file_type()
            .await
            .with_context(|| format!("Failed to inspect {}", entry.path().display()))?;

        if !file_type.is_file() {
            continue;
        }

        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if !is_managed_temp_artifact(&file_name) {
            continue;
        }

        tokio::fs::remove_file(entry.path())
            .await
            .with_context(|| {
                format!("Failed to remove temporary file {}", entry.path().display())
            })?;
        removed += 1;
    }

    Ok(removed)
}

async fn cleanup_download_artifacts(dir: &Path) -> Result<usize> {
    let spinner = tui::spinner("4/4 Cleaning temporary files...");
    let result = cleanup_managed_temp_files(dir).await;

    match result {
        Ok(removed) => {
            let message = if removed == 0 {
                "4/4 Cleanup complete".to_string()
            } else if removed == 1 {
                "4/4 Removed 1 temporary file".to_string()
            } else {
                format!("4/4 Removed {removed} temporary files")
            };
            tui::spinner_ok(&spinner, &message);
            Ok(removed)
        }
        Err(err) => {
            tui::spinner_err(&spinner, "4/4 Cleanup failed");
            Err(err)
        }
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

    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

fn is_managed_temp_artifact(file_name: &str) -> bool {
    let base_name = file_name
        .strip_suffix(".parts")
        .or_else(|| file_name.strip_suffix(".part"))
        .unwrap_or(file_name);

    base_name.starts_with("temp_audio_")
        || base_name.starts_with("temp_video_")
        || base_name.starts_with("clip_audio_")
        || base_name.starts_with("clip_video_")
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
    fn format_playlist_duration_matches_reference_style() {
        assert_eq!(format_playlist_duration(Some(59.0)), "0:59");
        assert_eq!(format_playlist_duration(Some(1_350.0)), "22:30");
        assert_eq!(format_playlist_duration(Some(6_300.0)), "1:45:00");
        assert_eq!(format_playlist_duration(None), "Unknown");
    }

    #[test]
    fn resolve_output_dir_expands_tilde_overrides() {
        let home = dirs::home_dir().expect("home dir should exist");
        let output = resolve_output_dir(Some(PathBuf::from("~/Downloads/vdl")), PathBuf::new())
            .expect("tilde output path should resolve");

        assert_eq!(output, home.join("Downloads/vdl"));
    }

    #[test]
    fn normalize_video_json_fills_required_defaults() {
        let mut value = json!({
            "id": "abc",
            "title": "TikTok clip",
            "formats": null,
            "thumbnails": null,
            "automatic_captions": null,
            "subtitles": null,
            "tags": null,
            "categories": null,
            "age_limit": null,
            "live_status": null,
            "playable_in_embed": null
        });

        normalize_video_json(&mut value);

        assert_eq!(value["age_limit"], json!(0));
        assert_eq!(value["live_status"], json!("not_live"));
        assert_eq!(value["playable_in_embed"], json!(true));
        assert_eq!(value["formats"], json!([]));
        assert_eq!(value["thumbnails"], json!([]));
        assert_eq!(value["automatic_captions"], json!({}));
        assert_eq!(value["subtitles"], json!({}));
        assert_eq!(value["tags"], json!([]));
        assert_eq!(value["categories"], json!([]));
    }

    #[test]
    fn detects_managed_temp_artifacts() {
        assert!(is_managed_temp_artifact("temp_audio_abc123.m4a"));
        assert!(is_managed_temp_artifact("temp_video_abc123.mp4.parts"));
        assert!(is_managed_temp_artifact("clip_video_abc123.webm"));
        assert!(!is_managed_temp_artifact("How_to_Rust.mp4"));
        assert!(!is_managed_temp_artifact("notes.parts"));
    }

    #[tokio::test]
    async fn cleanup_managed_temp_files_removes_only_managed_artifacts() {
        let dir = std::env::temp_dir().join(format!(
            "vdl-cleanup-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos()
        ));

        tokio::fs::create_dir_all(&dir)
            .await
            .expect("temp cleanup test directory should be created");
        tokio::fs::write(dir.join("temp_audio_one.m4a"), b"x")
            .await
            .expect("managed temp file should be created");
        tokio::fs::write(dir.join("temp_video_one.mp4.parts"), b"x")
            .await
            .expect("managed parts file should be created");
        tokio::fs::write(dir.join("final_video.mp4"), b"x")
            .await
            .expect("final video file should be created");

        let removed = cleanup_managed_temp_files(&dir)
            .await
            .expect("cleanup should succeed");

        assert_eq!(removed, 2);
        assert!(!dir.join("temp_audio_one.m4a").exists());
        assert!(!dir.join("temp_video_one.mp4.parts").exists());
        assert!(dir.join("final_video.mp4").exists());

        tokio::fs::remove_file(dir.join("final_video.mp4"))
            .await
            .expect("final test artifact should be removed");
        tokio::fs::remove_dir(&dir)
            .await
            .expect("temp cleanup test directory should be removed");
    }

    #[test]
    fn display_path_uses_tilde_for_home_relative_paths() {
        let home = dirs::home_dir().expect("home dir should exist");
        let path = home.join(".config/vdl/config.yaml");

        assert_eq!(display_path(&path), "~/.config/vdl/config.yaml");
    }
}

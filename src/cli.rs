use std::path::PathBuf;

use clap::{builder::PossibleValuesParser, Args, Parser, Subcommand};

/// Parses the top-level `vdl` command line and selects a subcommand to run.
///
/// # Examples
///
/// ```rust,ignore
/// let cli = Cli::parse_from(["vdl", "yt", "--url", "https://example.com/watch?v=123"]);
/// ```
#[derive(Debug, Parser)]
#[command(name = "vdl", about = "Video Downloader CLI")]
pub struct Cli {
    /// Selects which platform command or utility action should run.
    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Lists all supported platform and utility subcommands.
#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Downloads media from YouTube.
    #[command(visible_alias = "youtube", about = "Download from YouTube")]
    Yt(CommonArgs),
    /// Downloads media from TikTok.
    #[command(visible_alias = "tiktok", about = "Download from TikTok")]
    Tk(CommonArgs),
    /// Downloads media from Instagram.
    #[command(visible_alias = "instagram", about = "Download from Instagram")]
    Ig(CommonArgs),
    /// Downloads media from Twitter/X.
    #[command(visible_alias = "twitter", about = "Download from Twitter/X")]
    Tw(CommonArgs),
    /// Downloads audio from Spotify.
    #[command(visible_alias = "spotify", about = "Download from Spotify (audio)")]
    Sp(SpotifyArgs),
    /// Updates sandboxed helper binaries.
    #[command(about = "Update sandboxed yt-dlp and ffmpeg binaries")]
    Update,
    /// Prints the current config path and YAML contents.
    #[command(about = "Show current config file path and contents")]
    Config,
}

/// Captures the shared download flags used by YouTube, TikTok, Instagram, and Twitter/X.
#[derive(Debug, Clone, Args)]
pub struct CommonArgs {
    /// Supplies a direct media URL to download.
    #[arg(short = 'u', long, value_name = "URL", conflicts_with = "search")]
    pub url: Option<String>,
    /// Selects the preferred output quality when a platform supports it.
    #[arg(short = 'q', long, value_name = "Q", value_parser = quality_parser())]
    pub quality: Option<String>,
    /// Downloads only the audio stream.
    #[arg(short = 'a', long, conflicts_with = "video_only")]
    pub audio_only: bool,
    /// Downloads only the video stream without audio.
    #[arg(short = 'v', long, conflicts_with = "audio_only")]
    pub video_only: bool,
    /// Overrides the output container format for this run.
    #[arg(short = 'f', long, value_name = "FMT", value_parser = common_format_parser())]
    pub format: Option<String>,
    /// Overrides the destination directory for this run only.
    #[arg(short = 'o', long, value_name = "PATH")]
    pub output: Option<PathBuf>,
    /// Searches by query instead of using a direct URL.
    #[arg(short = 's', long, value_name = "QUERY", conflicts_with = "url")]
    pub search: Option<String>,
    /// Skips the confirmation prompt after metadata preview.
    #[arg(short = 'y', long)]
    pub yes: bool,
}

/// Captures the Spotify-specific flags for audio downloads.
#[derive(Debug, Clone, Args)]
pub struct SpotifyArgs {
    /// Supplies a direct Spotify URL to download.
    #[arg(short = 'u', long, value_name = "URL")]
    pub url: Option<String>,
    /// Overrides the destination directory for this run only.
    #[arg(short = 'o', long, value_name = "PATH")]
    pub output: Option<PathBuf>,
    /// Selects the audio container written to disk.
    #[arg(short = 'f', long, value_name = "FMT", value_parser = spotify_format_parser())]
    pub format: Option<String>,
    /// Skips the confirmation prompt after metadata preview.
    #[arg(short = 'y', long)]
    pub yes: bool,
}

fn quality_parser() -> PossibleValuesParser {
    PossibleValuesParser::new(["best", "1080", "720", "480", "360", "worst"])
}

fn common_format_parser() -> PossibleValuesParser {
    PossibleValuesParser::new(["mp4", "mkv", "webm", "mp3", "m4a", "opus"])
}

fn spotify_format_parser() -> PossibleValuesParser {
    PossibleValuesParser::new(["mp3", "m4a", "opus"])
}

#[cfg(test)]
mod tests {
    use clap::{CommandFactory, Parser};

    use super::*;

    #[test]
    fn top_level_help_lists_expected_subcommands() {
        let mut cmd = Cli::command();
        let mut buffer = Vec::new();

        cmd.write_long_help(&mut buffer)
            .expect("top-level help should render");

        let help = String::from_utf8(buffer).expect("help should be valid utf-8");

        assert!(help.contains("yt"));
        assert!(help.contains("youtube"));
        assert!(help.contains("tk"));
        assert!(help.contains("tiktok"));
        assert!(help.contains("ig"));
        assert!(help.contains("instagram"));
        assert!(help.contains("tw"));
        assert!(help.contains("twitter"));
        assert!(help.contains("sp"));
        assert!(help.contains("spotify"));
        assert!(help.contains("update"));
        assert!(help.contains("config"));
    }

    #[test]
    fn yt_help_lists_expected_flags() {
        let mut cmd = Cli::command();
        let mut buffer = Vec::new();

        cmd.find_subcommand_mut("yt")
            .expect("yt subcommand should exist")
            .write_long_help(&mut buffer)
            .expect("yt help should render");

        let help = String::from_utf8(buffer).expect("help should be valid utf-8");

        for flag in [
            "--url",
            "--quality",
            "--audio-only",
            "--video-only",
            "--format",
            "--output",
            "--search",
            "--yes",
        ] {
            assert!(help.contains(flag), "missing flag in help: {flag}");
        }
    }

    #[test]
    fn command_aliases_parse() {
        let cli = Cli::try_parse_from(["vdl", "youtube", "--url", "https://example.com"])
            .expect("youtube alias should parse");

        assert!(matches!(cli.command, Some(Commands::Yt(_))));
    }

    #[test]
    fn conflicting_audio_and_video_flags_error() {
        let err = Cli::try_parse_from(["vdl", "yt", "--audio-only", "--video-only"])
            .expect_err("conflicting audio/video flags should error");

        assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn conflicting_url_and_search_flags_error() {
        let err = Cli::try_parse_from([
            "vdl",
            "yt",
            "--url",
            "https://example.com",
            "--search",
            "rust",
        ])
        .expect_err("conflicting url/search flags should error");

        assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn invalid_quality_is_rejected() {
        let err = Cli::try_parse_from(["vdl", "yt", "--quality", "144"])
            .expect_err("invalid quality should error");

        assert_eq!(err.kind(), clap::error::ErrorKind::InvalidValue);
    }

    #[test]
    fn spotify_rejects_video_only_flag() {
        let err = Cli::try_parse_from(["vdl", "sp", "--video-only"])
            .expect_err("spotify should reject unsupported flags");

        assert_eq!(err.kind(), clap::error::ErrorKind::UnknownArgument);
    }
}

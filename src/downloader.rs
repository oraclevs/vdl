use anyhow::{bail, Context, Result};
use yt_dlp::Downloader;

use crate::{config::Config, sandbox};

/// Builds a sandboxed downloader rooted at the configured default output directory.
///
/// # Arguments
///
/// * `cfg` - Loaded application configuration that supplies the sandboxed binary paths and
///   default download directory.
///
/// # Returns
///
/// Returns a fully initialised [`yt_dlp::Downloader`].
///
/// # Errors
///
/// Returns an error if the downloader cannot be initialised with the configured binaries or
/// output directory.
///
/// # Examples
///
/// ```rust,ignore
/// let cfg = Config::load()?;
/// let downloader = build(&cfg).await?;
/// ```
pub async fn build(cfg: &Config) -> Result<Downloader> {
    let libs = sandbox::libraries(cfg);
    let output = cfg.download_path_expanded();
    let mut builder = Downloader::builder(libs, output);

    if let (Some(cookie_file), Some(browser)) = (
        cfg.cookies_file_expanded(),
        normalized_browser_name(cfg.cookies_from_browser.as_deref()),
    ) {
        bail!(
            "Config options cookies_file ({}) and cookies_from_browser ({browser}) are mutually exclusive",
            cookie_file.display()
        );
    }

    if let Some(cookie_file) = cfg.cookies_file_expanded() {
        builder = builder.with_cookies(cookie_file);
    }

    if let Some(browser) = normalized_browser_name(cfg.cookies_from_browser.as_deref()) {
        builder = builder.with_cookies_from_browser(browser);
    }

    builder
        .build()
        .await
        .context("Failed to initialise Downloader")
}

fn normalized_browser_name(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, PlatformQuality};

    #[test]
    fn normalizes_optional_browser_names() {
        assert_eq!(
            normalized_browser_name(Some(" chrome ")),
            Some("chrome".to_string())
        );
        assert_eq!(normalized_browser_name(Some("   ")), None);
        assert_eq!(normalized_browser_name(None), None);
    }

    #[test]
    fn cookie_file_path_expands_when_present() {
        let cfg = test_config();

        assert_eq!(
            cfg.cookies_file_expanded(),
            dirs::home_dir().map(|home| home.join(".config/vdl/cookies.txt"))
        );
    }

    fn test_config() -> Config {
        Config {
            download_path: "~/Downloads/vdl".to_string(),
            default_format: "mp4".to_string(),
            default_video_quality: "1080".to_string(),
            platform_quality: PlatformQuality {
                youtube: "1080".to_string(),
                tiktok: "720".to_string(),
                instagram: "720".to_string(),
                twitter: "720".to_string(),
                spotify: "best".to_string(),
            },
            bins_dir: "~/.local/share/vdl/bins".to_string(),
            cookies_file: Some("~/.config/vdl/cookies.txt".to_string()),
            cookies_from_browser: None,
            confirm_before_download: true,
            search_results_count: 8,
        }
    }
}

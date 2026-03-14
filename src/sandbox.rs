//! Manages the private `yt-dlp` and `ffmpeg` binaries used by `vdl`.
//!
//! Sandboxing in this project means the helper binaries live under the configured `bins_dir`
//! instead of the system `PATH`, so `vdl` can update and invoke them without modifying the
//! user's global toolchain.

use std::path::PathBuf;

use anyhow::{Context, Result};
use tokio::fs;
use yt_dlp::client::deps::{Libraries, LibraryInstaller};

use crate::{config::Config, downloader, tui};

/// Returns the expanded sandbox directory that stores `yt-dlp` and `ffmpeg`.
///
/// # Arguments
///
/// * `cfg` - Loaded application configuration.
///
/// # Returns
///
/// Returns the fully expanded sandbox directory path.
pub fn bins_dir(cfg: &Config) -> PathBuf {
    cfg.bins_dir_expanded()
}

/// Resolves the platform-specific path to the sandboxed `yt-dlp` executable.
///
/// # Arguments
///
/// * `cfg` - Loaded application configuration.
///
/// # Returns
///
/// Returns the absolute path to the managed `yt-dlp` binary.
pub fn ytdlp_path(cfg: &Config) -> PathBuf {
    bins_dir(cfg).join(executable_name("yt-dlp"))
}

/// Resolves the platform-specific path to the sandboxed `ffmpeg` executable.
///
/// # Arguments
///
/// * `cfg` - Loaded application configuration.
///
/// # Returns
///
/// Returns the absolute path to the managed `ffmpeg` binary.
pub fn ffmpeg_path(cfg: &Config) -> PathBuf {
    bins_dir(cfg).join(executable_name("ffmpeg"))
}

/// Builds the `yt-dlp` dependency descriptor used everywhere in the application.
///
/// # Arguments
///
/// * `cfg` - Loaded application configuration.
///
/// # Returns
///
/// Returns a [`yt_dlp::client::deps::Libraries`] value pointing at the sandboxed binaries.
pub fn libraries(cfg: &Config) -> Libraries {
    Libraries::new(ytdlp_path(cfg), ffmpeg_path(cfg))
}

/// Ensures the sandbox directory exists and downloads missing helper binaries.
///
/// # Arguments
///
/// * `cfg` - Loaded application configuration.
///
/// # Errors
///
/// Returns an error if the sandbox directory cannot be created or if either helper binary
/// cannot be downloaded.
///
/// # Examples
///
/// ```rust,ignore
/// let cfg = Config::load()?;
/// ensure_installed(&cfg).await?;
/// ```
pub async fn ensure_installed(cfg: &Config) -> Result<()> {
    let dir = bins_dir(cfg);
    fs::create_dir_all(&dir)
        .await
        .with_context(|| format!("Failed to create bins directory at {}", dir.display()))?;

    let installer = LibraryInstaller::new(dir);

    if !ytdlp_path(cfg).exists() {
        install_ytdlp(&installer)
            .await
            .context("Failed to ensure sandboxed yt-dlp is installed")?;
    }

    if !ffmpeg_path(cfg).exists() {
        install_ffmpeg(&installer)
            .await
            .context("Failed to ensure sandboxed ffmpeg is installed")?;
    }

    Ok(())
}

/// Updates the sandboxed `yt-dlp` binary and ensures `ffmpeg` is present afterwards.
///
/// # Arguments
///
/// * `cfg` - Loaded application configuration.
///
/// # Errors
///
/// Returns an error if the sandbox cannot be prepared or if the update/download steps fail.
///
/// # Examples
///
/// ```rust,ignore
/// let cfg = Config::load()?;
/// update_binaries(&cfg).await?;
/// ```
pub async fn update_binaries(cfg: &Config) -> Result<()> {
    let dir = bins_dir(cfg);
    fs::create_dir_all(&dir)
        .await
        .with_context(|| format!("Failed to create bins directory at {}", dir.display()))?;

    let installer = LibraryInstaller::new(dir);

    if !ytdlp_path(cfg).exists() {
        install_ytdlp(&installer)
            .await
            .context("Failed to ensure sandboxed yt-dlp is installed before update")?;
    }

    let pb = tui::spinner("Updating vdl dependencies...");
    let downloader = downloader::build(cfg)
        .await
        .context("Failed to initialise downloader for binary update")?;

    match downloader.update_downloader().await {
        Ok(()) => tui::spinner_ok(&pb, "yt-dlp updated successfully"),
        Err(err) => {
            tui::spinner_err(&pb, "Failed to update yt-dlp");
            return Err(err).context("Failed to update yt-dlp");
        }
    }

    if !ffmpeg_path(cfg).exists() {
        install_ffmpeg(&installer)
            .await
            .context("Failed to ensure sandboxed ffmpeg is installed after update")?;
    }

    Ok(())
}

fn executable_name(base: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("{base}.exe")
    } else {
        base.to_string()
    }
}

async fn install_ytdlp(installer: &LibraryInstaller) -> Result<()> {
    let pb = tui::spinner("Downloading sandboxed yt-dlp...");

    match installer.install_youtube(None).await {
        Ok(path) => {
            tui::spinner_ok(&pb, &format!("yt-dlp downloaded to {}", path.display()));
            Ok(())
        }
        Err(err) => {
            tui::spinner_err(&pb, "Failed to download yt-dlp");
            Err(err).context(format!(
                "Failed to install yt-dlp into {}",
                installer.destination.display()
            ))
        }
    }
}

async fn install_ffmpeg(installer: &LibraryInstaller) -> Result<()> {
    let pb = tui::spinner("Downloading sandboxed ffmpeg...");

    match installer.install_ffmpeg(None).await {
        Ok(path) => {
            tui::spinner_ok(&pb, &format!("ffmpeg downloaded to {}", path.display()));
            Ok(())
        }
        Err(err) => {
            tui::spinner_err(&pb, "Failed to download ffmpeg");
            Err(err).context(format!(
                "Failed to install ffmpeg into {}",
                installer.destination.display()
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, PlatformQuality};

    #[test]
    fn bins_dir_uses_expanded_config_path() {
        let cfg = test_config();

        assert_eq!(bins_dir(&cfg), cfg.bins_dir_expanded());
    }

    #[test]
    fn sandbox_binary_paths_use_platform_filenames() {
        let cfg = test_config();
        let expected_ytdlp = if cfg!(target_os = "windows") {
            "yt-dlp.exe"
        } else {
            "yt-dlp"
        };
        let expected_ffmpeg = if cfg!(target_os = "windows") {
            "ffmpeg.exe"
        } else {
            "ffmpeg"
        };

        assert_eq!(ytdlp_path(&cfg), bins_dir(&cfg).join(expected_ytdlp));
        assert_eq!(ffmpeg_path(&cfg), bins_dir(&cfg).join(expected_ffmpeg));
    }

    #[test]
    fn libraries_are_built_from_sandbox_paths() {
        let cfg = test_config();
        let libs = libraries(&cfg);

        assert_eq!(libs.youtube, ytdlp_path(&cfg));
        assert_eq!(libs.ffmpeg, ffmpeg_path(&cfg));
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
            cookies_file: None,
            cookies_from_browser: None,
            confirm_before_download: true,
            search_results_count: 8,
        }
    }
}

use std::path::PathBuf;

use anyhow::{Context, Result};
use tokio::fs;
use yt_dlp::client::deps::{Libraries, LibraryInstaller};

use crate::{config::Config, downloader, tui};

pub fn bins_dir(cfg: &Config) -> PathBuf {
    cfg.bins_dir_expanded()
}

pub fn ytdlp_path(cfg: &Config) -> PathBuf {
    bins_dir(cfg).join(executable_name("yt-dlp"))
}

pub fn ffmpeg_path(cfg: &Config) -> PathBuf {
    bins_dir(cfg).join(executable_name("ffmpeg"))
}

pub fn libraries(cfg: &Config) -> Libraries {
    Libraries::new(ytdlp_path(cfg), ffmpeg_path(cfg))
}

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
            confirm_before_download: true,
            search_results_count: 8,
        }
    }
}

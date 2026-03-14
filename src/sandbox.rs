//! Manages the private `yt-dlp` and `ffmpeg` binaries used by `vdl`.
//!
//! Sandboxing in this project means the helper binaries live under the configured `bins_dir`
//! instead of the system `PATH`, so `vdl` can update and invoke them without modifying the
//! user's global toolchain.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tokio::fs;
use yt_dlp::client::deps::{Libraries, LibraryInstaller};

use crate::{config::Config, downloader, tui};

/// Returns `true` when `vdl` is running inside a Termux session on Android.
///
/// # Returns
///
/// Returns `true` when the `TERMUX_VERSION` environment variable is present or when the
/// resolved home directory looks like a Termux-managed path.
pub fn is_termux() -> bool {
    is_termux_with(
        std::env::var("TERMUX_VERSION").ok().as_deref(),
        dirs::home_dir(),
    )
}

/// Applies Termux-specific environment variables before any network work begins.
///
/// # Examples
///
/// ```rust,ignore
/// if sandbox::is_termux() {
///     sandbox::apply_termux_env();
/// }
/// ```
pub fn apply_termux_env() {
    let termux_cert = "/data/data/com.termux/files/usr/etc/tls/cert.pem";
    if Path::new(termux_cert).exists() {
        std::env::set_var("SSL_CERT_FILE", termux_cert);
        std::env::set_var("REQUESTS_CA_BUNDLE", termux_cert);
    }

    let termux_prefix = "/data/data/com.termux/files/usr";
    if Path::new(termux_prefix).exists() {
        let termux_bin = format!("{termux_prefix}/bin");
        let current_path = std::env::var("PATH").unwrap_or_default();
        if !current_path.split(':').any(|segment| segment == termux_bin) {
            let new_path = if current_path.is_empty() {
                termux_bin
            } else {
                format!("{termux_bin}:{current_path}")
            };
            std::env::set_var("PATH", new_path);
        }
    }
}

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
        install_ytdlp(&installer, cfg.no_progress)
            .await
            .context("Failed to ensure sandboxed yt-dlp is installed")?;
    }

    if !ffmpeg_path(cfg).exists() {
        install_ffmpeg(&installer, cfg.no_progress)
            .await
            .context("Failed to ensure sandboxed ffmpeg is installed")?;
    }

    ensure_binary_permissions(cfg)?;

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
        install_ytdlp(&installer, cfg.no_progress)
            .await
            .context("Failed to ensure sandboxed yt-dlp is installed before update")?;
    }

    let pb = tui::spinner("Updating vdl dependencies...", cfg.no_progress);
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
        install_ffmpeg(&installer, cfg.no_progress)
            .await
            .context("Failed to ensure sandboxed ffmpeg is installed after update")?;
    }

    ensure_binary_permissions(cfg)?;

    Ok(())
}

fn executable_name(base: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("{base}.exe")
    } else {
        base.to_string()
    }
}

async fn install_ytdlp(installer: &LibraryInstaller, no_progress: bool) -> Result<()> {
    let pb = tui::spinner("Downloading sandboxed yt-dlp...", no_progress);

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

async fn install_ffmpeg(installer: &LibraryInstaller, no_progress: bool) -> Result<()> {
    let pb = tui::spinner("Downloading sandboxed ffmpeg...", no_progress);

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

fn is_termux_with(termux_version: Option<&str>, home_dir: Option<PathBuf>) -> bool {
    if termux_version.is_some() {
        return true;
    }

    home_dir
        .map(|home| home.to_string_lossy().contains("com.termux"))
        .unwrap_or(false)
}

fn ensure_binary_permissions(cfg: &Config) -> Result<()> {
    #[cfg(unix)]
    {
        set_executable_permissions_if_present(&ytdlp_path(cfg))?;
        set_executable_permissions_if_present(&ffmpeg_path(cfg))?;
    }

    Ok(())
}

#[cfg(unix)]
fn set_executable_permissions_if_present(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    if path.exists() {
        let mut permissions = std::fs::metadata(path)
            .with_context(|| format!("Failed to read permissions for {}", path.display()))?
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).with_context(|| {
            format!("Failed to set executable permissions on {}", path.display())
        })?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs::File;

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

    #[test]
    fn detects_termux_from_env_or_home_path() {
        assert!(is_termux_with(Some("0.118.0"), None));
        assert!(is_termux_with(
            None,
            Some(PathBuf::from("/data/data/com.termux/files/home"))
        ));
        assert!(!is_termux_with(None, Some(PathBuf::from("/home/occ"))));
    }

    #[cfg(unix)]
    #[test]
    fn executable_permissions_are_applied_when_binary_exists() {
        use std::os::unix::fs::PermissionsExt;

        let dir = unique_test_dir("chmod");
        std::fs::create_dir_all(&dir).expect("test dir should be created");
        let path = dir.join("yt-dlp");

        File::create(&path).expect("test binary should be created");
        let mut permissions = std::fs::metadata(&path)
            .expect("metadata should be readable")
            .permissions();
        permissions.set_mode(0o644);
        std::fs::set_permissions(&path, permissions).expect("permissions should be set");

        set_executable_permissions_if_present(&path).expect("chmod should succeed");

        let mode = std::fs::metadata(&path)
            .expect("metadata should be readable")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o755);

        std::fs::remove_dir_all(&dir).expect("test dir cleanup should succeed");
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
            termux_mode: false,
            no_progress: false,
        }
    }

    fn unique_test_dir(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();

        std::env::temp_dir().join(format!("vdl-sandbox-test-{name}-{nanos}"))
    }
}

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::sandbox;

const CONFIG_DIR_RELATIVE: &str = ".config/vdl";
const CONFIG_FILE_NAME: &str = "config.yaml";
const EXAMPLE_CONFIG: &str = include_str!("../config.example.yaml");

/// Represents the required user configuration loaded from `~/.config/vdl/config.yaml`.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Stores the default directory used for completed downloads.
    pub download_path: String,
    /// Selects the default output container when `--format` is omitted.
    pub default_format: String,
    /// Selects the fallback video quality when `--quality` is omitted.
    pub default_video_quality: String,
    /// Overrides the default quality per supported platform.
    pub platform_quality: PlatformQuality,
    /// Points at the sandbox directory where `yt-dlp` and `ffmpeg` binaries are stored.
    pub bins_dir: String,
    /// Optionally points at a Netscape cookie file for authenticated downloads.
    #[serde(default)]
    pub cookies_file: Option<String>,
    /// Optionally names a browser to extract cookies from for authenticated downloads.
    #[serde(default)]
    pub cookies_from_browser: Option<String>,
    /// Controls whether metadata preview confirmation is shown before downloads start.
    pub confirm_before_download: bool,
    /// Limits the number of interactive YouTube search results displayed to the user.
    pub search_results_count: usize,
    /// Enables Termux-specific runtime behavior when the application runs inside Termux.
    #[serde(default)]
    pub termux_mode: bool,
    /// Disables animated progress indicators in Termux and other non-interactive environments.
    #[serde(default)]
    pub no_progress: bool,
}

/// Stores per-platform quality preferences used when a command omits `--quality`.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlatformQuality {
    /// Sets the default quality override for YouTube downloads.
    pub youtube: String,
    /// Sets the default quality override for TikTok downloads.
    pub tiktok: String,
    /// Sets the default quality override for Instagram downloads.
    pub instagram: String,
    /// Sets the default quality override for Twitter/X downloads.
    pub twitter: String,
    /// Sets the default quality override for Spotify downloads.
    pub spotify: String,
}

impl Config {
    /// Loads the application configuration from the standard user config path.
    ///
    /// # Returns
    ///
    /// Returns the deserialised [`Config`] value with runtime overrides applied.
    ///
    /// # Errors
    ///
    /// Returns an error if the config path cannot be resolved, the file cannot be read,
    /// or the YAML contents are invalid.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let cfg = Config::load()?;
    /// println!("{}", cfg.download_path_expanded().display());
    /// ```
    pub fn load() -> Result<Config> {
        let path = config_path().context("Failed to resolve vdl config path")?;
        load_from_path(&path).context("Failed to load vdl config")
    }

    /// Ensures the user config file exists, creating it from the bundled example on first run.
    ///
    /// # Returns
    ///
    /// Returns `true` when a new config file was created and `false` when one already existed.
    ///
    /// # Errors
    ///
    /// Returns an error if the config path cannot be resolved, the parent directory cannot be
    /// created, or the example config cannot be written.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// if Config::ensure_exists()? {
    ///     println!("created config");
    /// }
    /// ```
    pub fn ensure_exists() -> Result<bool> {
        let path = config_path().context("Failed to resolve vdl config path")?;
        ensure_exists_at(&path).context("Failed to ensure vdl config exists")
    }

    /// Expands a leading `~` in [`Config::download_path`] to the current user's home directory.
    pub fn download_path_expanded(&self) -> PathBuf {
        expand_tilde(&self.download_path)
    }

    /// Expands a leading `~` in [`Config::bins_dir`] to the current user's home directory.
    pub fn bins_dir_expanded(&self) -> PathBuf {
        expand_tilde(&self.bins_dir)
    }

    /// Expands a leading `~` in [`Config::cookies_file`] when a cookie file is configured.
    pub fn cookies_file_expanded(&self) -> Option<PathBuf> {
        self.cookies_file
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(expand_tilde)
    }
}

/// Resolves the directory that contains `vdl` configuration files.
///
/// # Returns
///
/// Returns the fully qualified config directory path.
///
/// # Errors
///
/// Returns an error if the current user's home directory cannot be determined.
pub fn config_dir() -> Result<PathBuf> {
    let home = require_home_dir().context("Failed to resolve vdl config directory")?;
    Ok(config_dir_from_home(&home))
}

/// Resolves the full path to `config.yaml` in the standard config directory.
///
/// # Returns
///
/// Returns the fully qualified config file path.
///
/// # Errors
///
/// Returns an error if the config directory cannot be resolved.
pub fn config_path() -> Result<PathBuf> {
    Ok(config_dir()?.join(CONFIG_FILE_NAME))
}

fn config_dir_from_home(home: &Path) -> PathBuf {
    home.join(CONFIG_DIR_RELATIVE)
}

fn ensure_exists_at(path: &Path) -> Result<bool> {
    if path.exists() {
        return Ok(false);
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!("Failed to create config directory at {}", parent.display())
        })?;
    }

    fs::write(path, EXAMPLE_CONFIG)
        .with_context(|| format!("Failed to write config file to {}", path.display()))?;

    Ok(true)
}

fn load_from_path(path: &Path) -> Result<Config> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file at {}", path.display()))?;

    let mut cfg: Config = serde_yaml::from_str(&contents)
        .with_context(|| format!("Failed to parse config file at {}", path.display()))?;

    apply_runtime_overrides(&mut cfg, sandbox::is_termux());
    Ok(cfg)
}

fn apply_runtime_overrides(cfg: &mut Config, termux_detected: bool) {
    if termux_detected {
        cfg.termux_mode = true;
    }

    if cfg.termux_mode {
        cfg.no_progress = true;
    }
}

fn require_home_dir() -> Result<PathBuf> {
    dirs::home_dir().context("Failed to resolve home directory")
}

fn expand_tilde(path: &str) -> PathBuf {
    match dirs::home_dir() {
        Some(home) => expand_tilde_with_home(path, &home),
        None => PathBuf::from(path),
    }
}

fn expand_tilde_with_home(path: &str, home: &Path) -> PathBuf {
    if path == "~" {
        return home.to_path_buf();
    }

    if let Some(stripped) = path.strip_prefix("~/") {
        return home.join(stripped);
    }

    PathBuf::from(path)
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn parses_example_config() {
        let cfg: Config =
            serde_yaml::from_str(EXAMPLE_CONFIG).expect("example config should parse");

        assert_eq!(cfg.download_path, "~/Downloads/vdl");
        assert_eq!(cfg.default_format, "mp4");
        assert_eq!(cfg.default_video_quality, "1080");
        assert_eq!(cfg.platform_quality.youtube, "1080");
        assert_eq!(cfg.platform_quality.spotify, "best");
        assert_eq!(cfg.bins_dir, "~/.local/share/vdl/bins");
        assert_eq!(cfg.cookies_file, None);
        assert_eq!(cfg.cookies_from_browser, None);
        assert!(cfg.confirm_before_download);
        assert_eq!(cfg.search_results_count, 8);
        assert!(!cfg.termux_mode);
        assert!(!cfg.no_progress);
    }

    #[test]
    fn expands_tilde_paths() {
        let home = Path::new("/tmp/vdl-home");

        assert_eq!(
            expand_tilde_with_home("~/Downloads/vdl", home),
            PathBuf::from("/tmp/vdl-home/Downloads/vdl")
        );
        assert_eq!(
            expand_tilde_with_home("~", home),
            PathBuf::from("/tmp/vdl-home")
        );
        assert_eq!(
            expand_tilde_with_home("/var/tmp/vdl", home),
            PathBuf::from("/var/tmp/vdl")
        );
    }

    #[test]
    fn ensure_exists_writes_example_file_once() {
        let path = unique_config_path("ensure-exists");

        assert!(ensure_exists_at(&path).expect("first ensure_exists should create config"));
        assert_eq!(
            fs::read_to_string(&path).expect("config file should be readable"),
            EXAMPLE_CONFIG
        );
        assert!(
            !ensure_exists_at(&path).expect("second ensure_exists should detect existing config")
        );

        fs::remove_dir_all(
            path.parent()
                .and_then(Path::parent)
                .expect("test dir should exist"),
        )
        .expect("test dir cleanup should succeed");
    }

    #[test]
    fn load_reads_config_from_path() {
        let path = unique_config_path("load");

        ensure_exists_at(&path).expect("config file should be created");
        let cfg = load_from_path(&path).expect("config should load from disk");

        assert_eq!(
            cfg.download_path_expanded(),
            expand_tilde("~/Downloads/vdl")
        );
        assert_eq!(
            cfg.bins_dir_expanded(),
            expand_tilde("~/.local/share/vdl/bins")
        );
        assert_eq!(cfg.cookies_file_expanded(), None);
        assert!(!cfg.termux_mode);
        assert!(!cfg.no_progress);

        fs::remove_dir_all(
            path.parent()
                .and_then(Path::parent)
                .expect("test dir should exist"),
        )
        .expect("test dir cleanup should succeed");
    }

    #[test]
    fn termux_override_enables_no_progress() {
        let mut cfg: Config =
            serde_yaml::from_str(EXAMPLE_CONFIG).expect("example config should parse");

        apply_runtime_overrides(&mut cfg, true);

        assert!(cfg.termux_mode);
        assert!(cfg.no_progress);
    }

    fn unique_config_path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();

        std::env::temp_dir()
            .join(format!(
                "vdl-config-test-{name}-{}-{nonce}",
                std::process::id()
            ))
            .join(".config")
            .join("vdl")
            .join("config.yaml")
    }
}

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

const CONFIG_DIR_RELATIVE: &str = ".config/vdl";
const CONFIG_FILE_NAME: &str = "config.yaml";
const EXAMPLE_CONFIG: &str = include_str!("../config.example.yaml");

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub download_path: String,
    pub default_format: String,
    pub default_video_quality: String,
    pub platform_quality: PlatformQuality,
    pub bins_dir: String,
    pub confirm_before_download: bool,
    pub search_results_count: usize,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlatformQuality {
    pub youtube: String,
    pub tiktok: String,
    pub instagram: String,
    pub twitter: String,
    pub spotify: String,
}

impl Config {
    pub fn load() -> Result<Config> {
        let path = config_path()?;
        load_from_path(&path)
    }

    pub fn ensure_exists() -> Result<bool> {
        let path = config_path()?;
        ensure_exists_at(&path)
    }

    pub fn download_path_expanded(&self) -> PathBuf {
        expand_tilde(&self.download_path)
    }

    pub fn bins_dir_expanded(&self) -> PathBuf {
        expand_tilde(&self.bins_dir)
    }
}

pub fn config_dir() -> Result<PathBuf> {
    let home = require_home_dir()?;
    Ok(config_dir_from_home(&home))
}

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

    serde_yaml::from_str(&contents)
        .with_context(|| format!("Failed to parse config file at {}", path.display()))
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
        assert!(cfg.confirm_before_download);
        assert_eq!(cfg.search_results_count, 8);
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

        fs::remove_dir_all(
            path.parent()
                .and_then(Path::parent)
                .expect("test dir should exist"),
        )
        .expect("test dir cleanup should succeed");
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

use std::path::Path;

use anyhow::{Context, Result};
use tokio::process::Command;

use crate::{sandbox, tui};

use super::{display_path, load_config_or_create};

pub async fn run() -> Result<()> {
    let Some(cfg) = load_config_or_create()? else {
        return Ok(());
    };

    tui::print_header("Dependencies", "Updating");
    sandbox::update_binaries(&cfg).await?;

    let bins_dir = sandbox::bins_dir(&cfg);
    let ytdlp_path = sandbox::ytdlp_path(&cfg);
    let ffmpeg_path = sandbox::ffmpeg_path(&cfg);

    let ytdlp_version = read_ytdlp_version(&ytdlp_path)
        .await
        .unwrap_or_else(|_| "Unknown".to_string());
    let ffmpeg_version = read_ffmpeg_version(&ffmpeg_path)
        .await
        .unwrap_or_else(|_| "Unknown".to_string());

    tui::print_success("Updated vdl dependencies.");
    tui::print_info(&format!("  bins_dir : {}", display_path(&bins_dir)));
    tui::print_info(&format!(
        "  yt-dlp   : {ytdlp_version} ({})",
        display_path(&ytdlp_path)
    ));
    tui::print_info(&format!(
        "  ffmpeg   : {ffmpeg_version} ({})",
        display_path(&ffmpeg_path)
    ));

    Ok(())
}

async fn read_ytdlp_version(path: &Path) -> Result<String> {
    read_first_line(path, &["--version"])
        .await?
        .context("yt-dlp version output was empty")
}

async fn read_ffmpeg_version(path: &Path) -> Result<String> {
    let line = read_first_line(path, &["-version"]).await?;
    let version = line
        .as_deref()
        .and_then(parse_ffmpeg_version)
        .map(str::to_string);

    version.context("ffmpeg version output was empty")
}

async fn read_first_line(path: &Path, args: &[&str]) -> Result<Option<String>> {
    if !path.exists() {
        return Ok(None);
    }

    let output = Command::new(path)
        .args(args)
        .output()
        .await
        .with_context(|| format!("Failed to execute {}", path.display()))?;

    if !output.status.success() {
        return Ok(None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let text = if stdout.trim().is_empty() {
        stderr.as_ref()
    } else {
        stdout.as_ref()
    };

    Ok(text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_string))
}

fn parse_ffmpeg_version(line: &str) -> Option<&str> {
    line.strip_prefix("ffmpeg version ")?
        .split_whitespace()
        .next()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ffmpeg_version_from_banner() {
        assert_eq!(
            parse_ffmpeg_version("ffmpeg version 7.1.1-static Copyright (c)"),
            Some("7.1.1-static")
        );
    }

    #[test]
    fn rejects_unexpected_ffmpeg_banner() {
        assert_eq!(parse_ffmpeg_version("unexpected output"), None);
    }
}

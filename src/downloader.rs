use anyhow::{Context, Result};
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

    Downloader::builder(libs, output)
        .build()
        .await
        .context("Failed to initialise Downloader")
}

use anyhow::{Context, Result};
use yt_dlp::Downloader;

use crate::{config::Config, sandbox};

pub async fn build(cfg: &Config) -> Result<Downloader> {
    let libs = sandbox::libraries(cfg);
    let output = cfg.download_path_expanded();

    Downloader::builder(libs, output)
        .build()
        .await
        .context("Failed to initialise Downloader")
}

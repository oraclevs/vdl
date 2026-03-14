use anyhow::Result;

use crate::cli::SpotifyArgs;

use super::run_spotify;

pub async fn run(args: SpotifyArgs) -> Result<()> {
    run_spotify(args).await
}

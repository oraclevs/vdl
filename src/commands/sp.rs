use anyhow::Result;

use crate::cli::SpotifyArgs;

use super::run_spotify;

/// Runs the Spotify command for audio-only downloads.
///
/// This handler reads the Spotify-specific flags from [`SpotifyArgs`], prints the metadata
/// preview, prompts for confirmation when enabled, and writes the final audio file to disk.
///
/// # Arguments
///
/// * `args` - Parsed CLI arguments from `vdl sp`.
///
/// # Errors
///
/// Returns an error if configuration loading, metadata fetching, or audio download execution
/// fails.
pub async fn run(args: SpotifyArgs) -> Result<()> {
    run_spotify(args).await
}

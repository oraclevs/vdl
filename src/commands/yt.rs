use anyhow::Result;

use crate::cli::CommonArgs;

use super::{run_common_platform, Platform};

/// Runs the YouTube command, supporting direct URLs and interactive search.
///
/// This handler reads the shared platform flags from [`CommonArgs`], prints the metadata preview,
/// prompts for confirmation when enabled, and writes the final file to the configured output path.
///
/// # Arguments
///
/// * `args` - Parsed CLI arguments from `vdl yt`.
///
/// # Errors
///
/// Returns an error if configuration loading, metadata fetching, search, or download execution
/// fails.
pub async fn run(args: CommonArgs) -> Result<()> {
    run_common_platform(Platform::YouTube, args).await
}

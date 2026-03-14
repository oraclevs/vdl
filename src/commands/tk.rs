use anyhow::Result;

use crate::cli::CommonArgs;

use super::{run_common_platform, Platform};

pub async fn run(args: CommonArgs) -> Result<()> {
    run_common_platform(Platform::TikTok, args).await
}

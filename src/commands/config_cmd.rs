use std::fs;

use anyhow::{Context, Result};

use crate::{config, tui};

use super::display_path;

pub async fn run() -> Result<()> {
    let path = config::config_path()?;
    let display_path = display_path(&path);

    tui::print_header("Config", "Showing");

    if !path.exists() {
        tui::print_missing_config(&display_path);
        return Ok(());
    }

    let contents = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read config file at {}", path.display()))?;

    tui::print_config_path(&display_path);
    tui::print_yaml(&contents);
    Ok(())
}

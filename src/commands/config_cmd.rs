use anyhow::{Context, Result};

use crate::{config, tui};

use super::display_path;

/// Runs the config display command.
///
/// This handler resolves the expected config path, loads the effective configuration with runtime
/// overrides applied, and renders the resulting YAML when the file exists.
///
/// # Errors
///
/// Returns an error if the config path cannot be resolved, the file cannot be read, or the
/// effective config cannot be serialised back to YAML.
pub async fn run() -> Result<()> {
    let path = config::config_path().context("Failed to resolve vdl config path")?;
    let display_path = display_path(&path);

    tui::print_header("Config", "Showing");

    if !path.exists() {
        tui::print_missing_config(&display_path);
        return Ok(());
    }

    let cfg = config::Config::load()?;
    let contents = serde_yaml::to_string(&cfg).context("Failed to render effective config YAML")?;

    tui::print_config_path(&display_path);
    tui::print_yaml(&contents);
    Ok(())
}

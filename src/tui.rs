use std::time::Duration;

use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};

pub fn spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    let style = ProgressStyle::with_template("{spinner:.cyan} {msg}")
        .map(|style| style.tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]))
        .unwrap_or_else(|_| ProgressStyle::default_spinner());

    pb.set_style(style);
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(Duration::from_millis(80));
    pb
}

pub fn spinner_ok(pb: &ProgressBar, msg: &str) {
    pb.finish_with_message(format!("{} {msg}", "✓".green()));
}

pub fn spinner_err(pb: &ProgressBar, msg: &str) {
    pb.finish_with_message(format!("{} {msg}", "✗".red()));
}

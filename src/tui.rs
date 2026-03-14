use std::io::{self, Write};
use std::time::Duration;

use anyhow::{Context, Result};
use colored::Colorize;
use console::Term;
use dialoguer::{Confirm, Input, Select};
use indicatif::{ProgressBar, ProgressStyle};
use yt_dlp::model::Video;

const HEADER_WIDTH: usize = 60;
const LABEL_WIDTH: usize = 10;
const SPINNER_TICKS: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Represents a selectable search result rendered in the interactive picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResult {
    /// Stores the display title shown in the picker.
    pub title: String,
    /// Stores the channel, uploader, or account label shown in the picker.
    pub uploader: String,
    /// Stores the preformatted duration string shown in the picker.
    pub duration: String,
    /// Stores the platform-specific identifier used to build the final URL.
    pub id: String,
}

/// Starts an animated spinner for long-running terminal operations.
///
/// # Arguments
///
/// * `msg` - Message displayed next to the spinner.
///
/// # Returns
///
/// Returns a running [`indicatif::ProgressBar`] configured as a spinner.
///
/// # Examples
///
/// ```rust,ignore
/// let pb = spinner("Fetching video info...");
/// ```
pub fn spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    let style = ProgressStyle::with_template("{spinner:.cyan} {msg}")
        .map(|style| style.tick_strings(&SPINNER_TICKS))
        .unwrap_or_else(|_| ProgressStyle::default_spinner());

    pb.set_style(style);
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(Duration::from_millis(80));
    pb
}

/// Finishes a spinner with a green success message.
///
/// # Arguments
///
/// * `pb` - Spinner to finish.
/// * `msg` - Success message to display.
///
/// # Examples
///
/// ```rust,ignore
/// spinner_ok(&pb, "Fetched video info");
/// ```
pub fn spinner_ok(pb: &ProgressBar, msg: &str) {
    pb.finish_with_message(format!("{} {msg}", "✓".green()));
}

/// Finishes a spinner with a red error message.
///
/// # Arguments
///
/// * `pb` - Spinner to finish.
/// * `msg` - Error message to display.
///
/// # Examples
///
/// ```rust,ignore
/// spinner_err(&pb, "Failed to download");
/// ```
pub fn spinner_err(pb: &ProgressBar, msg: &str) {
    pb.finish_with_message(format!("{} {msg}", "✗".red()));
}

/// Creates a byte-oriented download progress bar.
///
/// # Arguments
///
/// * `total_bytes` - Expected size of the download in bytes.
/// * `filename` - Label shown next to the bar.
///
/// # Returns
///
/// Returns a started [`indicatif::ProgressBar`] configured for byte progress.
///
/// # Examples
///
/// ```rust,ignore
/// let pb = download_bar(512 * 1024 * 1024, "movie.mp4");
/// ```
pub fn download_bar(total_bytes: u64, filename: &str) -> ProgressBar {
    let pb = ProgressBar::new(total_bytes);
    let style = ProgressStyle::with_template(
        "{spinner:.cyan} {msg} [{bar:40.green/white}] {bytes}/{total_bytes} ({eta})",
    )
    .map(|style| style.tick_strings(&SPINNER_TICKS).progress_chars("█▓░"))
    .unwrap_or_else(|_| ProgressStyle::default_bar());

    pb.set_style(style);
    pb.set_length(total_bytes);
    pb.set_position(0);
    pb.set_message(filename.to_string());
    pb.enable_steady_tick(Duration::from_millis(80));
    pb
}

/// Updates a byte-oriented download progress bar.
///
/// # Arguments
///
/// * `pb` - Progress bar created by [`download_bar`].
/// * `downloaded` - Number of bytes already written.
/// * `total` - Total expected number of bytes.
///
/// # Examples
///
/// ```rust,ignore
/// update_download_bar(&pb, 128, 1024);
/// ```
pub fn update_download_bar(pb: &ProgressBar, downloaded: u64, total: u64) {
    pb.set_length(total);
    pb.set_position(downloaded);
}

/// Creates a percentage-based progress bar for APIs that report fractional progress.
///
/// # Arguments
///
/// * `filename` - Label shown next to the bar.
///
/// # Returns
///
/// Returns a started [`indicatif::ProgressBar`] configured for percentage progress.
///
/// # Examples
///
/// ```rust,ignore
/// let pb = progress_bar("episode.mp4");
/// ```
pub fn progress_bar(filename: &str) -> ProgressBar {
    let pb = download_bar(1000, filename);
    let style =
        ProgressStyle::with_template("{spinner:.cyan} {msg} [{bar:40.green/white}] {percent:>3}%")
            .map(|style| style.tick_strings(&SPINNER_TICKS).progress_chars("█▓░"))
            .unwrap_or_else(|_| ProgressStyle::default_bar());

    pb.set_style(style);
    pb.set_length(1000);
    pb.set_position(0);
    pb.set_message(filename.to_string());
    pb.enable_steady_tick(Duration::from_millis(80));
    pb
}

/// Updates a percentage-based progress bar from a fractional value.
///
/// # Arguments
///
/// * `pb` - Progress bar created by [`progress_bar`].
/// * `fraction` - Progress in the inclusive range `0.0..=1.0`.
///
/// # Examples
///
/// ```rust,ignore
/// update_progress_bar(&pb, 0.5);
/// ```
pub fn update_progress_bar(pb: &ProgressBar, fraction: f64) {
    let clamped = fraction.clamp(0.0, 1.0);
    update_download_bar(pb, (clamped * 1000.0).round() as u64, 1000);
}

/// Updates the label shown next to a progress indicator.
///
/// # Arguments
///
/// * `pb` - Progress bar created by [`progress_bar`] or [`download_bar`].
/// * `msg` - Message shown next to the progress indicator.
///
/// # Examples
///
/// ```rust,ignore
/// set_progress_message(&pb, "2/4 Downloading audio stream...");
/// ```
pub fn set_progress_message(pb: &ProgressBar, msg: &str) {
    pb.set_message(msg.to_string());
}

/// Advances a percentage-based progress bar to at least the given fraction.
///
/// # Arguments
///
/// * `pb` - Progress bar created by [`progress_bar`].
/// * `fraction` - Minimum progress in the inclusive range `0.0..=1.0`.
///
/// # Examples
///
/// ```rust,ignore
/// advance_progress_bar(&pb, 0.9);
/// ```
pub fn advance_progress_bar(pb: &ProgressBar, fraction: f64) {
    let target = (fraction.clamp(0.0, 1.0) * 1000.0).round() as u64;
    if target > pb.position() {
        update_progress_bar(pb, fraction);
    }
}

/// Clears an active progress indicator from the terminal.
///
/// # Arguments
///
/// * `pb` - Progress bar or spinner to clear.
///
/// # Examples
///
/// ```rust,ignore
/// clear_progress(&pb);
/// ```
pub fn clear_progress(pb: &ProgressBar) {
    pb.finish_and_clear();
}

/// Prints the standard metadata preview block for a fetched video.
///
/// # Arguments
///
/// * `video` - Video metadata returned by the downloader.
///
/// # Examples
///
/// ```rust,ignore
/// print_metadata(&video);
/// ```
pub fn print_metadata(video: &Video) {
    print_metadata_block(&metadata_entries(video));
}

/// Prints the Spotify-specific metadata preview block.
///
/// # Arguments
///
/// * `video` - Video metadata returned by the downloader.
///
/// # Examples
///
/// ```rust,ignore
/// print_spotify_metadata(&video);
/// ```
pub fn print_spotify_metadata(video: &Video) {
    let entries = vec![
        ("Title", video.title.clone()),
        (
            "Artist",
            video
                .uploader
                .clone()
                .unwrap_or_else(|| "Unknown".to_string()),
        ),
        ("Duration", format_duration(video.duration)),
        (
            "URL",
            video
                .webpage_url
                .clone()
                .unwrap_or_else(|| "Unknown".to_string()),
        ),
    ];

    print_metadata_block(&entries);
}

/// Prompts the user to confirm whether a download should continue.
///
/// # Arguments
///
/// * `video_title` - Title inserted into the confirmation question.
///
/// # Returns
///
/// Returns `true` when the user confirms the download.
///
/// # Errors
///
/// Returns an error if terminal input cannot be read.
///
/// # Examples
///
/// ```rust,ignore
/// if confirm_download("Example Title")? {
///     // start download
/// }
/// ```
pub fn confirm_download(video_title: &str) -> Result<bool> {
    confirm_download_on(video_title, &Term::stderr())
}

/// Opens an interactive selector for a list of search results.
///
/// # Arguments
///
/// * `results` - Search results to display, followed by an implicit `Cancel` option.
///
/// # Returns
///
/// Returns `Some(index)` for a selected video or `None` when the user cancels.
///
/// # Errors
///
/// Returns an error if terminal input cannot be read.
///
/// # Examples
///
/// ```rust,ignore
/// let selection = select_search_result(&results)?;
/// ```
pub fn select_search_result(results: &[SearchResult]) -> Result<Option<usize>> {
    select_search_result_on(results, &Term::stderr())
}

/// Prints a cyan section header for the current platform action.
///
/// # Arguments
///
/// * `platform` - Platform name such as `YouTube`.
/// * `action` - Active operation such as `Searching`.
///
/// # Examples
///
/// ```rust,ignore
/// print_header("YouTube", "Downloading");
/// ```
pub fn print_header(platform: &str, action: &str) {
    let mut stdout = io::stdout().lock();
    let _ = writeln!(stdout, "{}", header_line(platform, action).cyan());
}

/// Prints an informational line to standard output.
///
/// # Arguments
///
/// * `msg` - Message text to print.
///
/// # Examples
///
/// ```rust,ignore
/// print_info("No results found.");
/// ```
pub fn print_info(msg: &str) {
    let mut stdout = io::stdout().lock();
    let _ = writeln!(stdout, "{msg}");
}

/// Prints a success line prefixed with a green checkmark.
///
/// # Arguments
///
/// * `msg` - Message text to print.
///
/// # Examples
///
/// ```rust,ignore
/// print_success("Saved to: /tmp/video.mp4");
/// ```
pub fn print_success(msg: &str) {
    let mut stdout = io::stdout().lock();
    let _ = writeln!(stdout, "{} {}", "✓".green(), msg.green());
}

/// Prints a warning line in yellow.
///
/// # Arguments
///
/// * `msg` - Message text to print.
///
/// # Examples
///
/// ```rust,ignore
/// print_warning("No config file found.");
/// ```
pub fn print_warning(msg: &str) {
    let mut stdout = io::stdout().lock();
    let _ = writeln!(stdout, "{}", msg.yellow());
}

/// Prints the first-run bootstrap message after creating a new config file.
///
/// # Arguments
///
/// * `config_path` - Human-friendly path shown to the user.
///
/// # Examples
///
/// ```rust,ignore
/// print_first_run("~/.config/vdl/config.yaml");
/// ```
pub fn print_first_run(config_path: &str) {
    print_warning("  Welcome to vdl! No config found.");
    print_info(&format!("  Config created at: {config_path}"));
    print_info("  Please edit it to set your download path, then run vdl again.");
}

/// Prints the resolved config file path in a highlighted style.
///
/// # Arguments
///
/// * `path` - Config path to display.
///
/// # Examples
///
/// ```rust,ignore
/// print_config_path("~/.config/vdl/config.yaml");
/// ```
pub fn print_config_path(path: &str) {
    let mut stdout = io::stdout().lock();
    let _ = writeln!(stdout, "Config path: {}", path.cyan());
}

/// Prints the message shown when the config file does not yet exist.
///
/// # Arguments
///
/// * `path` - Expected config path.
///
/// # Examples
///
/// ```rust,ignore
/// print_missing_config("~/.config/vdl/config.yaml");
/// ```
pub fn print_missing_config(path: &str) {
    print_warning("No config file found.");
    let mut stdout = io::stdout().lock();
    let _ = writeln!(stdout, "Expected path: {}", path.cyan());
}

/// Prints YAML contents with simple syntax-aware colouring.
///
/// # Arguments
///
/// * `contents` - Raw YAML text to render.
///
/// # Examples
///
/// ```rust,ignore
/// print_yaml("download_path: ~/Downloads/vdl");
/// ```
pub fn print_yaml(contents: &str) {
    let mut stdout = io::stdout().lock();

    for line in contents.lines() {
        let _ = writeln!(stdout, "{}", format_yaml_line(line));
    }
}

/// Prompts the user for a single line of text input.
///
/// # Arguments
///
/// * `prompt` - Prompt text shown before reading user input.
///
/// # Returns
///
/// Returns the text entered by the user.
///
/// # Errors
///
/// Returns an error if terminal input cannot be read.
///
/// # Examples
///
/// ```rust,ignore
/// let query = prompt_input("Enter search query")?;
/// ```
pub fn prompt_input(prompt: &str) -> Result<String> {
    Input::<String>::new()
        .with_prompt(prompt)
        .interact_text()
        .context("Failed to read input")
}

fn confirm_download_on(video_title: &str, term: &Term) -> Result<bool> {
    Confirm::new()
        .with_prompt(format!("Download \"{video_title}\"?"))
        .default(true)
        .report(false)
        .interact_on(term)
        .context("Failed to read download confirmation")
}

fn select_search_result_on(results: &[SearchResult], term: &Term) -> Result<Option<usize>> {
    let items = search_result_items(results);
    let selection = Select::new()
        .with_prompt("Select a video to download")
        .items(&items)
        .default(0)
        .report(false)
        .interact_on_opt(term)
        .context("Failed to read search selection")?;

    Ok(selection.and_then(|index| (index < results.len()).then_some(index)))
}

fn metadata_entries(video: &Video) -> Vec<(&'static str, String)> {
    vec![
        ("Title", video.title.clone()),
        (
            "Uploader",
            video
                .uploader
                .clone()
                .unwrap_or_else(|| "Unknown".to_string()),
        ),
        ("Duration", format_duration(video.duration)),
        (
            "Views",
            video
                .view_count
                .map(format_number)
                .unwrap_or_else(|| "Unknown".to_string()),
        ),
        ("Upload", format_upload_date(video.upload_date)),
        (
            "URL",
            video
                .webpage_url
                .clone()
                .unwrap_or_else(|| "Unknown".to_string()),
        ),
    ]
}

fn print_metadata_block(entries: &[(&'static str, String)]) {
    let value_width = entries
        .iter()
        .map(|(_, value)| value.chars().count())
        .max()
        .unwrap_or(0);
    let inner_width = LABEL_WIDTH + value_width + 5;
    let border = "─".repeat(inner_width);
    let mut stdout = io::stdout().lock();

    let _ = writeln!(stdout, "┌{border}┐");
    for (label, value) in entries {
        let label = format!("{label:<LABEL_WIDTH$}");
        let value = format!("{value:<value_width$}");
        let _ = writeln!(stdout, "│  {}: {} │", label.cyan().bold(), value.white());
    }
    let _ = writeln!(stdout, "└{border}┘");
}

fn search_result_items(results: &[SearchResult]) -> Vec<String> {
    let mut items = results
        .iter()
        .enumerate()
        .map(|(index, result)| {
            format!(
                "[{}] {} — {} ({})",
                index + 1,
                result.title,
                result.uploader,
                result.duration
            )
        })
        .collect::<Vec<_>>();

    items.push("Cancel".to_string());
    items
}

fn header_line(platform: &str, action: &str) -> String {
    let base = format!("── vdl · {platform} · {action} ");
    let base_width = base.chars().count();

    if base_width >= HEADER_WIDTH {
        base
    } else {
        format!("{base}{}", "─".repeat(HEADER_WIDTH - base_width))
    }
}

fn format_yaml_line(line: &str) -> String {
    if line.trim().is_empty() {
        return String::new();
    }

    if line.trim_start().starts_with('#') {
        return line.dimmed().to_string();
    }

    if let Some((indent, key, rest)) = split_yaml_mapping_line(line) {
        if rest.is_empty() {
            format!("{indent}{}:", key.yellow())
        } else {
            format!("{indent}{}:{}", key.yellow(), rest.white())
        }
    } else {
        line.white().to_string()
    }
}

fn split_yaml_mapping_line(line: &str) -> Option<(&str, &str, &str)> {
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') {
        return None;
    }

    let indent_len = line.len() - trimmed.len();
    let indent = &line[..indent_len];
    let (key, rest) = trimmed.split_once(':')?;

    if key.trim().is_empty() {
        None
    } else {
        Some((indent, key, rest))
    }
}

fn format_duration(duration: Option<i64>) -> String {
    let Some(total_seconds) = duration else {
        return "Unknown".to_string();
    };

    if total_seconds < 0 {
        return "Unknown".to_string();
    }

    format_clock_duration(total_seconds)
}

fn format_clock_duration(total_seconds: i64) -> String {
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

fn format_number(value: i64) -> String {
    let is_negative = value < 0;
    let digits = i128::from(value).abs().to_string();
    let mut formatted = String::with_capacity(digits.len() + digits.len() / 3);

    for (index, ch) in digits.chars().rev().enumerate() {
        if index != 0 && index % 3 == 0 {
            formatted.push(',');
        }
        formatted.push(ch);
    }

    let mut formatted = formatted.chars().rev().collect::<String>();
    if is_negative {
        formatted.insert(0, '-');
    }

    formatted
}

fn format_upload_date(timestamp: Option<i64>) -> String {
    let Some(timestamp) = timestamp else {
        return "Unknown".to_string();
    };

    let days = timestamp.div_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

fn civil_from_days(days_since_unix_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_unix_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };

    if month <= 2 {
        year += 1;
    }

    (year as i32, month as u32, day as u32)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use console::Term;
    use yt_dlp::model::video::{ExtractorInfo, Version};
    use yt_dlp::model::DrmStatus;

    use super::*;

    #[test]
    fn spinner_finishes_with_success_message() {
        let pb = spinner("Working...");
        spinner_ok(&pb, "Done");

        assert!(pb.is_finished());
    }

    #[test]
    fn download_bar_tracks_message_and_progress() {
        let pb = download_bar(1024, "demo.mp4");
        update_download_bar(&pb, 256, 2048);

        assert_eq!(pb.message(), "demo.mp4");
        assert_eq!(pb.position(), 256);
        assert_eq!(pb.length(), Some(2048));
    }

    #[test]
    fn progress_bar_tracks_fractional_progress() {
        let pb = progress_bar("demo.mp4");
        update_progress_bar(&pb, 0.375);

        assert_eq!(pb.message(), "demo.mp4");
        assert_eq!(pb.length(), Some(1000));
        assert_eq!(pb.position(), 375);
    }

    #[test]
    fn metadata_entries_use_expected_fallbacks_and_formats() {
        let video = test_video();
        let entries = metadata_entries(&video);

        assert_eq!(entries[0], ("Title", "How to Learn Rust".to_string()));
        assert_eq!(entries[1], ("Uploader", "Unknown".to_string()));
        assert_eq!(entries[2], ("Duration", "1:01:01".to_string()));
        assert_eq!(entries[3], ("Views", "1,234,567".to_string()));
        assert_eq!(entries[4], ("Upload", "2025-01-15".to_string()));
        assert_eq!(
            entries[5],
            ("URL", "https://youtube.com/watch?v=xyz".to_string())
        );
    }

    #[test]
    fn search_results_include_cancel_entry() {
        let items = search_result_items(&[
            SearchResult {
                title: "Rust Async Explained".to_string(),
                uploader: "No Boilerplate".to_string(),
                duration: "22:30".to_string(),
                id: "abc".to_string(),
            },
            SearchResult {
                title: "Tokio Tutorial".to_string(),
                uploader: "Jon Gjengset".to_string(),
                duration: "1:45:00".to_string(),
                id: "def".to_string(),
            },
        ]);

        assert_eq!(
            items,
            vec![
                "[1] Rust Async Explained — No Boilerplate (22:30)".to_string(),
                "[2] Tokio Tutorial — Jon Gjengset (1:45:00)".to_string(),
                "Cancel".to_string(),
            ]
        );
    }

    #[test]
    fn header_line_is_padded_to_target_width() {
        let header = header_line("YouTube", "Searching");

        assert_eq!(header.chars().count(), HEADER_WIDTH);
        assert!(header.starts_with("── vdl · YouTube · Searching "));
    }

    #[test]
    fn upload_date_formats_unix_timestamp() {
        assert_eq!(format_upload_date(Some(1_736_899_200)), "2025-01-15");
        assert_eq!(format_upload_date(None), "Unknown");
    }

    #[test]
    fn duration_formats_with_hours() {
        assert_eq!(format_duration(Some(59)), "0:59");
        assert_eq!(format_duration(Some(635)), "10:35");
        assert_eq!(format_duration(Some(3_661)), "1:01:01");
        assert_eq!(format_duration(None), "Unknown");
    }

    #[test]
    fn number_format_adds_commas() {
        assert_eq!(format_number(0), "0");
        assert_eq!(format_number(12_345), "12,345");
        assert_eq!(format_number(1_234_567), "1,234,567");
    }

    #[test]
    fn yaml_mapping_lines_are_split_correctly() {
        assert_eq!(
            split_yaml_mapping_line("  youtube:   1080"),
            Some(("  ", "youtube", "   1080"))
        );
        assert_eq!(
            split_yaml_mapping_line("platform_quality:"),
            Some(("", "platform_quality", ""))
        );
        assert_eq!(split_yaml_mapping_line("# comment"), None);
    }

    #[test]
    fn yaml_line_formatter_preserves_structure() {
        colored::control::set_override(false);

        assert_eq!(
            format_yaml_line("download_path: ~/Downloads/vdl"),
            "download_path: ~/Downloads/vdl"
        );
        assert_eq!(format_yaml_line("  youtube:   1080"), "  youtube:   1080");
        assert_eq!(format_yaml_line(""), "");
    }

    #[test]
    #[ignore = "interactive prompt smoke test"]
    fn interactive_confirm_accepts_default() {
        let accepted =
            confirm_download_on("Interactive Test", &Term::stderr()).expect("confirm should work");

        assert!(accepted);
    }

    #[test]
    #[ignore = "interactive prompt smoke test"]
    fn interactive_select_returns_first_entry() {
        let selection = select_search_result_on(
            &[SearchResult {
                title: "Rust Async Explained".to_string(),
                uploader: "No Boilerplate".to_string(),
                duration: "22:30".to_string(),
                id: "abc".to_string(),
            }],
            &Term::stderr(),
        )
        .expect("selection should work");

        assert_eq!(selection, Some(0));
    }

    fn test_video() -> Video {
        Video {
            id: "xyz".to_string(),
            title: "How to Learn Rust".to_string(),
            thumbnail: None,
            description: None,
            availability: None,
            upload_date: Some(1_736_899_200),
            duration: Some(3_661),
            duration_string: Some("1:01:01".to_string()),
            webpage_url: Some("https://youtube.com/watch?v=xyz".to_string()),
            language: None,
            media_type: None,
            is_live: None,
            was_live: None,
            release_timestamp: None,
            release_year: None,
            view_count: Some(1_234_567),
            like_count: None,
            comment_count: None,
            channel: None,
            channel_id: None,
            channel_url: None,
            channel_follower_count: None,
            uploader: None,
            uploader_id: None,
            uploader_url: None,
            channel_is_verified: None,
            formats: Vec::new(),
            thumbnails: Vec::new(),
            automatic_captions: HashMap::new(),
            subtitles: HashMap::new(),
            chapters: Vec::new(),
            heatmap: None,
            tags: Vec::new(),
            categories: Vec::new(),
            age_limit: 0,
            has_drm: Some(DrmStatus::No),
            live_status: "not_live".to_string(),
            playable_in_embed: true,
            extractor_info: ExtractorInfo {
                extractor: "youtube".to_string(),
                extractor_key: "Youtube".to_string(),
            },
            version: Version {
                version: "2024.01.01".to_string(),
                current_git_head: None,
                release_git_head: Some("abc123".to_string()),
                repository: "yt-dlp/yt-dlp".to_string(),
            },
        }
    }
}

<div align="center">

# vdl

**A fast, interactive terminal video downloader**

[![Crates.io](https://img.shields.io/crates/v/vdl.svg)](https://crates.io/crates/vdl)
[![Downloads](https://img.shields.io/crates/d/vdl.svg)](https://crates.io/crates/vdl)
[![License](https://img.shields.io/badge/license-GPL--3.0--or--later-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)

</div>

---

> Download videos from YouTube, TikTok, Instagram, Twitter, and Spotify
> from your terminal — with interactive search, quality selection,
> metadata preview, and real-time progress bars.

## Features

- **Interactive search** — search YouTube by keyword and pick
  from results with arrow keys, no URL needed
- **Quality control** — choose 1080p, 720p, 480p, audio-only,
  or video-only per download
- **Metadata preview** — see title, uploader, duration, and views
  before committing to a download
- **Real-time progress** — animated spinner and byte-level
  progress bar for every download
- **Five platforms** — YouTube, TikTok, Instagram, Twitter/X,
  and Spotify in one tool
- **Sandboxed binaries** — yt-dlp and ffmpeg are stored in
  `~/.local/share/vdl/bins` and never touch your system PATH
- **Zero config to start** — sensible defaults, with a YAML
  config file for everything customisable

## Installation

```bash
cargo install vdl
```

That's it. On first run, `vdl` creates a config file at
`~/.config/vdl/config.yaml`. The first download or update command then
downloads `yt-dlp` and `ffmpeg` automatically into
`~/.local/share/vdl/bins/`.

On Termux, `vdl` auto-detects the environment, applies the certificate
bundle path for HTTPS downloads, and falls back to plain progress lines
instead of animated spinners.

**Prerequisites:** Rust 1.75 or newer. Install from https://rustup.rs.

## Quick Start

```bash
# Download a YouTube video at 1080p
vdl yt --url https://www.youtube.com/watch?v=dQw4w9WgXcQ

# Search YouTube interactively — no URL needed
vdl yt --search "rust async programming"

# Audio only (saves as .mp3)
vdl yt --url <url> --audio-only

# Download from TikTok
vdl tk --url https://www.tiktok.com/@user/video/123

# Download from Twitter/X
vdl tw --url https://twitter.com/user/status/456

# Download from Instagram
vdl ig --url https://www.instagram.com/p/ABC123/

# Download audio from Spotify
vdl sp --url https://open.spotify.com/track/...

# Skip the confirmation prompt
vdl yt --url <url> --yes
```

## Commands

| Command | Alias | Description |
|---------|-------|-------------|
| `vdl yt` | `youtube` | Download from YouTube |
| `vdl tk` | `tiktok` | Download from TikTok |
| `vdl ig` | `instagram` | Download from Instagram |
| `vdl tw` | `twitter` | Download from Twitter/X |
| `vdl sp` | `spotify` | Download audio from Spotify |
| `vdl update` | — | Update sandboxed yt-dlp and ffmpeg |
| `vdl config` | — | Show config file path and contents |

### Flags (yt, tk, ig, tw)

| Flag | Short | Description |
|------|-------|-------------|
| `--url <URL>` | `-u` | Direct URL to download |
| `--search <QUERY>` | `-s` | Search and select interactively (YouTube only) |
| `--quality <Q>` | `-q` | `best` \| `1080` \| `720` \| `480` \| `360` \| `worst` |
| `--audio-only` | `-a` | Download audio stream only (.mp3) |
| `--video-only` | `-v` | Download video stream only (no audio) |
| `--format <FMT>` | `-f` | `mp4` \| `mkv` \| `webm` \| `mp3` \| `m4a` \| `opus` |
| `--output <PATH>` | `-o` | Override output directory for this run |
| `--yes` | `-y` | Skip the confirmation prompt |

## Configuration

The config file lives at `~/.config/vdl/config.yaml`.
Run `vdl config` to see the current path and contents.

```yaml
# Where downloaded files are saved
download_path: ~/Downloads/vdl

# Default container format: mp4 | mkv | webm | mp3 | m4a | opus
default_format: mp4

# Default video quality: best | 1080 | 720 | 480 | 360 | worst
default_video_quality: 1080

# Per-platform quality overrides
platform_quality:
  youtube:   1080
  tiktok:    720
  instagram: 720
  twitter:   720
  spotify:   best

# Path where yt-dlp and ffmpeg are stored (sandboxed)
bins_dir: ~/.local/share/vdl/bins

# Optional Netscape cookies file for authenticated downloads
cookies_file: null

# Optional browser to import cookies from: chrome | firefox | brave | edge
cookies_from_browser: null

# Show metadata and prompt before every download
confirm_before_download: true

# Number of search results to display
search_results_count: 8

# Auto-detected on Android / Termux
termux_mode: false

# Forced on automatically when termux_mode is true
no_progress: false
```

For Instagram, Spotify, and login-gated YouTube videos, set either
`cookies_file` or `cookies_from_browser` before downloading.

## License

Copyright (C) 2026 oraclevs

Licensed under the GNU General Public License v3.0 or later.
See [LICENSE](LICENSE) for details.

This project uses [yt-dlp](https://github.com/yt-dlp/yt-dlp) and
[ffmpeg](https://ffmpeg.org), which are downloaded automatically
and stored in a local sandbox directory.

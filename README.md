# vibeveil

[![CI](https://github.com/jxoesneon/vibeveil/actions/workflows/ci.yml/badge.svg)](https://github.com/jxoesneon/vibeveil/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)

A music-driven dynamic wallpaper and desktop theming daemon for Linux (Wayland/X11).

vibeveil monitors active audio playback over MPRIS (Spotify, Feishin, Amberol, MPD, etc.) and synchronizes your desktop wallpaper, lockscreen colors, and compositor window borders to match the album art and vibe of the currently playing track.

## Features

- **Event-Driven MPRIS Listener:** Uses asynchronous D-Bus signals (`zbus`) to subscribe to playback state changes at 0% idle CPU.
- **Perceptual Color Science:** Computes dominant color clusters and evaluates Oklab delta-E perceptual color distance against indexed wallpapers.
- **Flexible Match Modes:**
  - `hybrid`: Evaluates regex rulebooks first, falls back to Oklab color distance, and generates procedural canvases when no match meets the threshold.
  - `color-distance`: Pure Oklab color matching.
  - `rulebook`: Deterministic user-defined regex rules mapping artist/album/title/genre to specific tags or files.
  - `procedural-canvas`: Dynamically synthesizes a blurred ambient background with album art.
  - `vinyl`: Procedurally generates a vinyl record canvas with concentric micro-grooves and album art center label.
  - `acoustic`: Mood classification via Last.fm, MusicBrainz, and local heuristic fallbacks with disk caching.
- **Compositor Integration:**
  - Native Hyprland border theming and Noctalia Material You integration.
  - Support for live hardware-accelerated video wallpapers via `mpvpaper` (VA-API / gpu-next).
  - Generic command backend (`swww`, `swaybg`, `feh`, etc.).
  - Dynamic lockscreen color synchronization for `hyprlock`.
- **Desktop Notifications:** Asynchronous notifications over session D-Bus with album art icons and wallpaper names.
- **Interactive Switcher:** Built-in `vibeveil menu` command that pipes indexed wallpapers into `rofi`, `wofi`, or `dmenu`.

## Installation

### Arch Linux (AUR)

Source package:
```bash
yay -S vibeveil
```

Precompiled binary:
```bash
yay -S vibeveil-bin
```

### Flatpak

```bash
flatpak-builder --user --install --force-clean build-dir packaging/flatpak/org.vibeveil.VibeVeil.yaml
```

### Nix / NixOS

Run directly with Flakes:
```bash
nix run github:jxoesneon/vibeveil -- daemon
```

Install into profile:
```bash
nix profile install github:jxoesneon/vibeveil
```

NixOS module example:
```nix
# In configuration.nix:
imports = [ "${vibeveil-src}/packaging/nix/module.nix" ];

services.vibeveil = {
  enable = true;
  settings.general.media_pool = "~/Pictures/Wallpapers";
};
```

### From Source

Requirements:
- Rust 1.75+ (edition 2024 compatible)
- `pkg-config`, `libdbus-1-dev`

```bash
git clone https://github.com/jxoesneon/vibeveil.git
cd vibeveil
cargo build --release
install -Dm755 target/release/vibeveil ~/.local/bin/vibeveil
install -Dm644 packaging/systemd/vibeveil.service ~/.config/systemd/user/vibeveil.service
systemctl --user daemon-reload
systemctl --user enable --now vibeveil.service
```

## CLI Usage

```bash
# Display active player, track metadata, and album art status
vibeveil status

# Perform a one-shot dry run match against the media pool
vibeveil match

# Match and immediately apply the wallpaper and system theme
vibeveil match --apply

# Open interactive selector in rofi, wofi, or dmenu
vibeveil menu

# Index wallpaper directory and compute Oklab color profiles
vibeveil index

# Run background listener daemon
vibeveil daemon

# Generate shell completions (bash, zsh, fish)
vibeveil completions bash > ~/.local/share/bash-completion/completions/vibeveil
```

## Configuration

Default location: `~/.config/vibeveil/config.toml`. Generate a default template with `vibeveil init-config`.

```toml
[general]
media_pool = "~/Pictures/Wallpapers"
default_wallpaper = "~/Pictures/Wallpapers/default.png"
on_pause = "restore-default" # "restore-default" | "pause-playback" | "keep-last"
extensions = ["mp4", "mkv", "webm", "png", "jpg", "jpeg", "webp"]
desktop_notifications = true

[strategy]
mode = "hybrid" # "hybrid" | "color-distance" | "rulebook" | "vinyl" | "acoustic"
color_space = "oklab"
max_acceptable_delta_e = 42.0
procedural_fallback = true
debounce_ms = 250

[compositor]
backend = "hyprland-noctalia"
trigger_hyprland_reload = true
trigger_noctalia_theming = true
sync_hyprlock = true
hyprlock_colors_path = "~/.config/hypr/hyprlock-colors.conf"
on_match_exec = "notify-send 'VibeVeil' 'Applied {file} for {title}'"

[[rules]]
match_field = "any"
pattern = "(?i)(synthwave|cyber|electro)"
target_tag_or_path = "cyberpunk"

[[rules]]
match_field = "any"
pattern = "(?i)(metal|rock|battle|doom)"
target_tag_or_path = "metal"

[[rules]]
match_field = "any"
pattern = "(?i)(lo-?fi|chill|ambient|nature)"
target_tag_or_path = "nature"
```

## Documentation

Detailed technical documentation and architecture decision records:
- [Product Requirements Document (PRD)](docs/PRD.md)
- [System Architecture Document (SAD)](docs/SYSTEM_ARCHITECTURE.md)
- [Configuration Specification](docs/CONFIG_SPECIFICATION.md)
- [Color Science & Oklab Derivation](docs/COLOR_SCIENCE.md)
- [Operational Runbook & Troubleshooting](docs/RUNBOOK.md)
- [Architecture Decision Records (ADRs)](docs/adr/)

## License

Dual-licensed under either of:
- MIT License ([LICENSE-MIT](LICENSE-MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))

at your option.

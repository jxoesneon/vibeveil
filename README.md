# 🌌 VibeVeil

[![CI](https://github.com/jxoesneon/vibeveil/actions/workflows/ci.yml/badge.svg)](https://github.com/jxoesneon/vibeveil/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2024%20edition-orange.svg)](https://www.rust-lang.org)
[![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20Wayland-informational.svg)](https://wayland.freedesktop.org)

> **Universal Music-Driven Dynamic Wallpaper & Semantic Desktop Theming Engine**  
> *Architected in safe, asynchronous Rust for Linux and Wayland.*

---

## ⚡ Overview

**VibeVeil** watches your active media playback (Spotify or any MPRIS-compliant player) and dynamically transforms your desktop wallpapers, lockscreen colors, and window borders to match the auditory and aesthetic mood of the music.

The matching engine is **one level removed**: it has no hardcoded dependencies on specific wallpapers. It operates on an abstract semantic pipeline capable of analyzing **any** directory tree of images, videos, or shaders using color science (Oklab $\Delta E$), metadata rulebooks, acoustic classifiers, or procedural ambient canvases.

---

## 📦 Installation & Packaging

### Arch Linux (AUR)
```bash
# Install from source:
yay -S vibeveil

# Or install precompiled binary:
yay -S vibeveil-bin
```

### Flatpak
```bash
flatpak-builder --user --install --force-clean build-dir packaging/flatpak/org.vibeveil.VibeVeil.yaml
```

### Nix / NixOS Flake
```bash
# Run directly with flakes:
nix run github:jxoesneon/vibeveil -- daemon

# Install to user profile:
nix profile install github:jxoesneon/vibeveil
```
Or import `packaging/nix/module.nix` into your `configuration.nix`:
```nix
services.vibeveil = {
  enable = true;
  settings.general.media_pool = "~/Pictures/Wallpapers";
};
```

### Universal Installer Script
```bash
curl -sSf https://raw.githubusercontent.com/jxoesneon/vibeveil/main/install.sh | bash
# Or from local clone:
./install.sh
```

### Build from Source
```bash
cargo build --release
install -Dm755 target/release/vibeveil ~/.local/bin/vibeveil
systemctl --user enable --now packaging/systemd/vibeveil.service
```

---

## 🚀 Key Features

* **Zero-Allocation MPRIS D-Bus Client (`zbus`):** Event-driven subscription to player state changes with zero continuous CPU usage.
* **Perceptual Color Science (`palette` + `image`):** Computes dominant color clusters and Oklab $\Delta E$ perceptual color distance against wallpaper color profiles.
* **Extensible Rulebook Engine:** Map custom regex patterns against artists, tracks, or genres to target specific tags or folders.
* **Native Desktop Notifications:** Zero-dependency async toasts over session D-Bus displaying track title, artist, wallpaper name, and album art icon.
* **Direct Hyprlock Palette Synchronization:** Automatically writes extracted hex color variables (`$primary`, `$secondary`, `$surface`, `$accent`) to `~/.config/hypr/hyprlock-colors.conf`.
* **Interactive Rofi / Wofi Switcher Menu (`vibeveil menu`):** Pick from your indexed media pool interactively via Rofi/Wofi with instant wallpaper and color application.
* **Acoustic Mood & Vibe Classifier:** Multi-tier fallback (`Last.fm -> MusicBrainz -> Local heuristic`) resolving energy and valence profiles for dynamic asset mapping.
* **Procedural Vinyl Canvas Mode (`mode = "vinyl"`):** Dynamic hardware-accelerated synthesis of 1080p vinyl records with micro-grooves, lighting sheen, and circular album art labels.
* **Compositor Backends:** Native support for Hyprland, Noctalia Material You M3 color extraction, and hardware-accelerated `mpvpaper` live videos.
* **Smart Pause / Fallback:** Automatically restores your signature default wallpaper (e.g. *Empoleon*) when playback pauses.

---

## 🛠️ CLI Usage

```bash
# Display currently playing Spotify track & album art URL
vibeveil status

# Match the active song once against your media pool
vibeveil match

# Match and immediately apply wallpaper & update theme
vibeveil match --apply

# Re-index any folder of wallpapers with perceptual color profiles
vibeveil index

# Launch the continuous background daemon
vibeveil daemon

# Launch daemon targeting an arbitrary folder of wallpapers
vibeveil daemon --pool ~/Pictures/Wallpapers/Anime
```

---

## ⚙️ Configuration (`~/.config/vibeveil/config.toml`)

```toml
[general]
media_pool = "/home/eduardo/Pictures/Wallpapers"
default_wallpaper = "/home/eduardo/Pictures/Wallpapers/Live/Pokemon/empoleon.mp4"
on_pause = "restore-default" # "restore-default" | "pause-playback" | "keep-last"
extensions = ["mp4", "mkv", "webm", "png", "jpg", "jpeg", "webp"]

[strategy]
mode = "hybrid" # "hybrid" | "color-distance" | "rulebook" | "procedural-canvas"
color_space = "oklab"
max_acceptable_delta_e = 45.0
procedural_fallback = true

[compositor]
backend = "hyprland-noctalia"
trigger_hyprland_reload = true
trigger_noctalia_theming = true

[[rules]]
match_field = "any"
pattern = "(?i)(synthwave|cyber|electro|night)"
target_tag_or_path = "cyberpunk_lucy"

[[rules]]
match_field = "any"
pattern = "(?i)(metal|rock|battle|doom|epic)"
target_tag_or_path = "groudon"

[[rules]]
match_field = "any"
pattern = "(?i)(lo-?fi|chill|ambient|nature)"
target_tag_or_path = "ghibli_nature"
```

---

## 📚 Institutional Documentation Suite (Document-Driven Development)

Comprehensive specifications, architectural decision records, and operational guides are maintained in the [`docs/`](docs/) repository:

* 📋 [**Product Requirements Document (PRD)**](docs/PRD.md): Vision, user personas, functional/non-functional requirements, edge cases, and acceptance criteria.
* 🏛️ [**System Architecture Document (SAD)**](docs/SYSTEM_ARCHITECTURE.md): Hexagonal architecture, subsystem decoupling, sequence diagrams, and concurrency model.
* ⚙️ [**Configuration Specification**](docs/CONFIG_SPECIFICATION.md): TOML schema definitions, strategy tuning, rulebook syntax, and reference rices.
* 🎨 [**Perceptual Color Science**](docs/COLOR_SCIENCE.md): Theoretical and mathematical foundations of Oklab $\Delta E$, gamma linearization, and multi-cluster palette weighting.
* 📖 [**Operational Runbook**](docs/RUNBOOK.md): Systemd deployment, troubleshooting matrix, diagnostic commands, and maintenance routines.

### Architecture Decision Records (ADRs)
* [**ADR-0001**](docs/adr/ADR-0001-rust-runtime.md): Adoption of Safe Async Rust as Core Daemon Runtime
* [**ADR-0002**](docs/adr/ADR-0002-zbus-async-mpris.md): Native Async D-Bus Integration via `zbus`
* [**ADR-0003**](docs/adr/ADR-0003-oklab-perceptual-distance.md): Selection of Oklab for Perceptual Color Matching
* [**ADR-0004**](docs/adr/ADR-0004-decoupled-matching-traits.md): Decoupled Strategy Pattern ("One Level Removed")
* [**ADR-0005**](docs/adr/ADR-0005-compositor-abstraction.md): Compositor Abstraction & Noctalia Material You Pipeline

*Maintained by Jose Eduardo Rojas Jimenez.*

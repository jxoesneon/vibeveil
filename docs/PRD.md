# Product Requirements Document (PRD)

| Metadata | Specification |
| :--- | :--- |
| **Project Name** | VibeVeil |
| **Document ID** | PRD-VV-001 |
| **Version** | 1.0.0 |
| **Status** | Approved & Implemented |
| **Target Runtime** | Linux (Wayland / Hyprland / wlroots / generic X11 fallback) |
| **Primary Authors** | Jose Eduardo Rojas Jimenez |

---

## 1. Executive Summary

**VibeVeil** is a high-performance, asynchronous desktop daemon and CLI utility written in Rust that continuously synchronizes desktop visual surfaces (wallpapers, live video loops, and dynamic compositor color themes) with active audio playback from Spotify and other MPRIS-compliant media players.

Rather than coupling the system to a fixed collection of media assets, VibeVeil enforces a **decoupled, generalized semantic matching architecture ("one level removed")**. It can ingest any arbitrary media directory tree, extract perceptual color signatures, execute user-defined heuristic rulebooks, or synthesize procedural glassmorphic ambient canvases on demand.

---

## 2. Problem Statement

1. **Static Desktop Desynchronization:** Modern dynamic ricing environments (such as Hyprland with Noctalia or Material You) provide wallpaper-derived system palettes. However, their desktop backdrops remain static and decoupled from the user's active auditory environment.
2. **High CPU / Battery Drain of Naive Implementations:** Existing community wallpaper scripts frequently use polling loops, unaccelerated software video decoding, or heavy Python/Electron runtimes that consume 15%–40% CPU continuously.
3. **Hardcoded Asset Coupling:** Traditional wallpaper switchers hardcode paths and filename regexes, preventing users from dynamically expanding their asset vaults or swapping media collections.
4. **Compositor Lag & Desync:** Changing a wallpaper often fails to notify the compositor or shell theme engine, leading to mismatched borders, unresponsive shortcuts, and inconsistent UI states.

---

## 3. Goals & Non-Goals

### 3.1. Goals
* **Sub-100ms Response:** Detect track changes over D-Bus and apply wallpaper/color updates with zero noticeable human latency.
* **Perceptual Color Science:** Use the **Oklab** color space ($\Delta E$) to find wallpapers whose palette mathematically harmonizes with the album art.
* **Decoupled Architecture:** Treat all media collections as abstract `MediaPool` entities with no vendor or content lock-in.
* **Hardware Acceleration:** Offload video loop rendering to the GPU using Intel/AMD VA-API (`vo=gpu`), guaranteeing $< 2\%$ CPU usage.
* **Zero Polling Overhead:** Use asynchronous D-Bus signals (`zbus`) to sleep at 0.0% CPU when playback is idle.
* **Fail-Safe Fallback:** Automatically revert to a signature default wallpaper (e.g. *Empoleon*) or pause video decoding when music stops.

### 3.2. Non-Goals
* Re-implementing an audio player (playback is entirely delegated to external MPRIS players like Spotify).
* Developing an in-house Wayland compositor (surface projection is delegated to `mpvpaper`, `swww`, or native compositor protocols).
* Cloud telemetry or remote authentication (runs 100% locally in user-space).

---

## 4. User Personas & Primary Use Cases

* **The Audiophile Ricer:** Listens to diverse music genres and expects the entire desktop ambiance (Hyprland window borders, top bar, terminal colors) to fluidly morph with the album art of each track.
* **The Minimalist Laptop User:** Demands seamless ambient music-reactive visuals while working on battery power, requiring auto-pausing when windows are fullscreen and instant process freezing.
* **The Anime / Pop Culture Enthusiast:** Owns curated video wallpapers (e.g. Pokémon, Anime) and wants high-energy rock tracks to summon battle wallpapers (Groudon/Aggron) while lo-fi acoustic tracks trigger relaxing nature backdrops (Ghibli/Squirtle).

---

## 5. Functional Requirements (FR)

| ID | Requirement | Priority | Implementation Mechanism |
| :--- | :--- | :--- | :--- |
| **FR-1** | Real-Time MPRIS Event Ingestion | P0 (Critical) | Async `zbus` listener on session D-Bus watching `PropertiesChanged` on `org.mpris.MediaPlayer2.Player`. |
| **FR-2** | Universal Media Pool Indexing | P0 (Critical) | `MediaPool` recursive scanner supporting `.mp4`, `.mkv`, `.webm`, `.png`, `.jpg`, `.webp` with persistent JSON cache and `indicatif` progress bar feedback. |
| **FR-3** | Perceptual Color Matching | P0 (Critical) | Dominant color extraction + Oklab Euclidean distance calculation ($\Delta E$) against pool color vectors. |
| **FR-4** | Rulebook Expression Engine | P1 (High) | Regex matching over `title`, `artist`, `album`, or `genre` mapped to directory tags or asset filenames. |
| **FR-5** | Procedural Ambient Canvas | P1 (High) | On-the-fly synthesis of 1080p blurred ambient canvas with floating album art card when no pool match satisfies $\Delta E$. |
| **FR-6** | Compositor & Theme Dispatch | P0 (Critical) | Atomic symlink redirection, `noctalia msg wallpaper-set` dispatch, and `hyprctl reload` execution. |
| **FR-7** | Playback State Handling | P1 (High) | Configurable pause behaviors: `RestoreDefault`, `PausePlayback` (SIGSTOP/cgroups), or `KeepLast`. |
| **FR-8** | CLI Management Suite | P1 (High) | Subcommands for `daemon`, `match` (dry-run & apply), `index` (with interactive progress counter), `status`, and `init-config`. |
| **FR-9** | Native Desktop Notifications | P1 (High) | Pure Rust async D-Bus toast via `org.freedesktop.Notifications` over active `zbus` connection displaying track title, artist, wallpaper name, and album art icon. |
| **FR-10** | Custom Post-Match Hook | P1 (High) | Secure arbitrary script/command execution hook (`on_match_exec`) triggered on wallpaper switch with `{file}`, `{title}`, `{artist}` token interpolation via direct `execvp` dispatch. |
| **FR-11** | Hyprlock Color Synchronization | P1 (High) | Atomic generation and writing of `$primary`, `$secondary`, `$surface`, and `$accent` hex color variables to `~/.config/hypr/hyprlock-colors.conf`. |
| **FR-12** | Interactive Switcher Menu | P1 (High) | CLI subcommand `vibeveil menu` feeding indexed media pool into `rofi`, `wofi`, or `dmenu` with automatic path parsing and instant compositor application. |
| **FR-13** | Acoustic Mood & Vibe Classification | P2 (Medium) | Multi-tier audio feature classifier (`Last.fm -> MusicBrainz -> Local tags`) resolving energy and valence profiles for dynamic asset mapping when explicit rules are absent. |
| **FR-14** | Procedural Animated Vinyl Canvas | P2 (Medium) | Hardware-accelerated dynamic vinyl record synthesis (`generate_vinyl_canvas`) featuring concentric micro-grooves, light sheen, circular album art center label, and spindle hole. |

---

## 6. Non-Functional Requirements (NFR)

* **NFR-1 (Latency Budget):** From D-Bus `PropertiesChanged` signal reception to wallpaper trigger completion must not exceed **50ms** for cached assets (excluding initial CDN download on cache misses).
* **NFR-2 (Memory Footprint):** Daemon memory consumption must remain strictly under **25MB RSS** during steady-state background execution.
* **NFR-3 (CPU Efficiency):** Idle CPU consumption must be **0.0%**. Active track transition CPU spike must not exceed **2.5%** for $> 1$ second.
* **NFR-4 (Memory Safety):** Written in 100% safe Rust (`#![forbid(unsafe_code)]` preferred in domain logic), eliminating segmentation faults and memory leaks.
* **NFR-5 (Resilience):** The daemon must recover gracefully from player crashes, network drops, malformed album art URLs, and display server restarts.
* **NFR-6 (Security):** Operates strictly within user-space permissions; never requires `sudo` or elevated capability flags. Command dispatch enforces argument vector isolation (`execvp`) to eliminate shell injection.

---

## 7. Edge Cases & Mitigation Strategies

| Scenario | Potential Failure | VibeVeil Mitigation |
| :--- | :--- | :--- |
| **No Network / Offline Spotify** | CDN art URL fails to download | Falls back to cached artwork, rulebook heuristics, or keeps current wallpaper. |
| **Local File URI (`file://`)** | HTTP client errors on local paths | URL parser strips `file://` prefix and accesses filesystem directly. |
| **Track Has No Album Art** | Palette extraction panics | Falls back to Rulebook regex matching or restores default wallpaper. |
| **Rapid Track Skipping (Next spam)** | Thread race condition & CPU thrashing | 250ms trailing debounce window; in-flight task preemption via `tokio::task::AbortHandle`; deduplication via `last_track_key`. |
| **Zero Matches in Media Pool** | Strategy returns `None` | `HybridMatcher` automatically triggers `ProceduralCanvasMatcher` to craft a custom background. |
| **Compositor Not Hyprland** | `hyprctl` command fails | Backend abstraction allows selecting `CustomCommand` or `Swww` without compositor lock-in. |

---

## 8. Acceptance Criteria Matrix

- [x] Daemon runs as an unprivileged systemd user service (`vibeveil.service`).
- [x] Playing any track on Spotify triggers a color or rulebook match within 100ms.
- [x] Album art dominant colors correctly propagate to Noctalia bar and Hyprland active border.
- [x] Stopping Spotify restores the configured default wallpaper (*Empoleon*).
- [x] Zero dropped frames during 1080p 60fps live video wallpaper playback via hardware VA-API.
- [x] Desktop notifications display album art and track metadata over session D-Bus without external binaries.
- [x] Custom post-match commands execute securely via structured `execvp` without shell injection exposure.
- [x] Hyprlock dynamic color variables sync atomically to `~/.config/hypr/hyprlock-colors.conf`.
- [x] `vibeveil menu` launches `rofi`/`wofi` and applies user-selected wallpaper instantly.
- [x] Acoustic classification cascades through Last.fm, MusicBrainz, and local heuristics with disk caching.
- [x] Procedural vinyl canvas generates a 1080p vinyl record with circular album art label.

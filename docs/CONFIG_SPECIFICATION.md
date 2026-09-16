# Configuration Specification

| Metadata | Specification |
| :--- | :--- |
| **Project Name** | VibeVeil |
| **Document ID** | CFG-VV-003 |
| **Version** | 1.0.0 |
| **Status** | Approved & Implemented |
| **Format** | TOML (Tom's Obvious Minimal Language) v1.0 |
| **Default Location** | `~/.config/vibeveil/config.toml` |
| **Environment Override** | `VIBEVEIL_CONFIG=/path/to/custom.toml` |
| **Primary Authors** | Jose Eduardo Rojas Jimenez |

---

## 1. Top-Level Structure

A valid `config.toml` contains four primary configuration tables:

```toml
[general]
# Asset storage, fallback behaviors, and file extensions

[strategy]
# Matching algorithm selection and perceptual color thresholds

[compositor]
# Wayland/X11 backend target and theme pipeline hooks

[[rules]]
# User-defined heuristic mappings from track metadata to asset tags
```

---

## 2. Table Specifications

### 2.1. `[general]` Table

| Key | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| `media_pool` | String (Path) | `~/Pictures/Wallpapers` | Absolute or tilde-prefixed path to directory tree containing wallpapers or video loops. |
| `default_wallpaper` | String (Path) | *(Empoleon.mp4)* | Wallpaper to restore when audio playback pauses or stops. |
| `on_pause` | String (Enum) | `"restore-default"` | Behavior when playback pauses: `"restore-default"`, `"pause-playback"`, or `"keep-last"`. |
| `extensions` | Array of Strings | `["mp4", "mkv", "webm", "png", "jpg", "jpeg", "webp", "gif"]` | File extensions ingested during recursive directory scans. |
| `max_cache_mb` | Integer | `200` | Maximum disk capacity in megabytes for cached album art before LRU pruning occurs. |
| `desktop_notifications` | Boolean | `true` | When `true`, dispatches desktop notifications via D-Bus (`org.freedesktop.Notifications`) showing track info, album art icon, and matched wallpaper. |

### 2.2. `[strategy]` Table

| Key | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| `mode` | String (Enum) | `"hybrid"` | Match strategy: `"hybrid"`, `"color-distance"`, `"rulebook"`, `"procedural-canvas"`, `"acoustic"`, or `"vinyl"`. |
| `color_space` | String (Enum) | `"oklab"` | Perceptual metric space: `"oklab"` or `"cielab"`. |
| `max_acceptable_delta_e` | Float | `45.0` | Maximum Euclidean color distance cutoff for considering two palettes harmonized. |
| `procedural_fallback` | Boolean | `true` | When `true`, automatically synthesizes an ambient canvas if no pool asset matches. |
| `debounce_ms` | Integer | `250` | Trailing debounce delay in milliseconds for rapid track skipping events. |

### 2.3. `[compositor]` Table

| Key | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| `backend` | String (Enum) | `"hyprland-noctalia"` | Compositor target: `"hyprland-noctalia"` or `"command"` (with built-in preset aliases `"swww"` and `"mpvpaper"`). |
| `custom_command` | String (Template) | `""` | Command executed when `backend = "command"`. Supports `{file}` substitution. Uses direct argument vector dispatch (`execvp`) to eliminate shell injection risks. |
| `trigger_hyprland_reload` | Boolean | `true` | Dispatches `hyprctl reload` to repaint active window borders on color changes. |
| `trigger_noctalia_theming` | Boolean | `true` | Invokes `noctalia msg wallpaper-set` to update top bar, Kitty, and GTK palettes. |
| `on_match_exec` | Optional String | `None` | Arbitrary custom post-match hook executed on every wallpaper switch (e.g. `wal -i '{file}'`). Supports `{file}`, `{title}`, and `{artist}` token interpolation via direct `execvp` dispatch. |
| `sync_hyprlock` | Boolean | `false` | When `true`, atomically syncs dynamic color variables (`$primary`, `$secondary`, `$surface`, `$accent`) to Hyprlock's config directory on color extraction. |
| `hyprlock_colors_path` | String (Path) | `~/.config/hypr/hyprlock-colors.conf` | Target configuration file written atomically when `sync_hyprlock = true`. |

### 2.4. `[acoustic]` Table

Configures mood and vibe classification for tracks lacking explicit rulebook definitions:

| Key | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| `enabled` | Boolean | `false` | Enables external or heuristic acoustic classification prior to color distance matching. |
| `provider` | String (Enum) | `"lastfm"` | Primary acoustic metadata source: `"lastfm"`, `"musicbrainz"`, or `"local"`. Note: Spotify `/v1/audio-features` was permanently deprecated on Nov 27, 2024. |
| `fallback` | Boolean | `true` | When `true`, automatically falls back through provider tiers (`Last.fm -> MusicBrainz -> Local tags`) if a query fails. |
| `lastfm_api_key` | Optional String | `None` | Last.fm 32-character API key. If absent, classifier seamlessly cascades to MusicBrainz open API and local heuristics. |
| `cache_dir` | String (Path) | `~/.cache/vibeveil/acoustic` | Directory storing persistent SHA-256 keyed JSON responses to minimize network requests. |

### 2.5. `[[rules]]` Array

Users can define arbitrary priority rules evaluated from top to bottom before color distance calculation:

| Key | Type | Options | Description |
| :--- | :--- | :--- | :--- |
| `match_field` | String (Enum) | `"artist"`, `"title"`, `"album"`, `"genre"`, `"any"` | The track metadata attribute tested against the regex pattern. |
| `pattern` | String (Regex) | *(Valid Rust regex)* | Case-insensitive regex tested against the metadata string (e.g. `"(?i)(metal\|rock)"`). |
| `target_tag_or_path` | String | *(Tag / Name)* | Name or path tag in the `MediaPool` to activate on match. |

---

## 3. Reference Configurations

### 3.1. Standard Hybrid Rice (Recommended Default)
```toml
[general]
media_pool = "/home/eduardo/Pictures/Wallpapers"
default_wallpaper = "/home/eduardo/Pictures/Wallpapers/Live/Pokemon/empoleon.mp4"
on_pause = "restore-default"
extensions = ["mp4", "mkv", "webm", "png", "jpg", "jpeg", "webp"]

[strategy]
mode = "hybrid"
color_space = "oklab"
max_acceptable_delta_e = 42.0
procedural_fallback = true

[compositor]
backend = "hyprland-noctalia"
trigger_hyprland_reload = true
trigger_noctalia_theming = true

[[rules]]
match_field = "any"
pattern = "(?i)(synthwave|cyber|electro|night|techno)"
target_tag_or_path = "cyberpunk_lucy"

[[rules]]
match_field = "any"
pattern = "(?i)(metal|rock|battle|doom|heavy)"
target_tag_or_path = "groudon"

[[rules]]
match_field = "any"
pattern = "(?i)(lo-?fi|chill|ambient|nature|piano)"
target_tag_or_path = "ghibli_nature"

[[rules]]
match_field = "any"
pattern = "(?i)(anime|j-?pop|fast|electric|speed)"
target_tag_or_path = "demon_slayer_zenitsu"
```

### 3.2. Pure Perceptual Color Matching (No Rules)
```toml
[general]
media_pool = "~/Pictures/Wallpapers/Curated"
on_pause = "keep-last"

[strategy]
mode = "color-distance"
max_acceptable_delta_e = 35.0
procedural_fallback = true

[compositor]
backend = "hyprland-noctalia"
```

### 3.3. Generic Compositor Setup (`swww` / Sway / Niri)
```toml
[general]
media_pool = "~/Pictures/Wallpapers"

[strategy]
mode = "hybrid"

[compositor]
backend = "command"
custom_command = "swww img --transition-type wipe --transition-duration 1 '{file}'"
```

### 3.4. Full Ricing Suite with Acoustic Classifier & Hyprlock Sync
```toml
[general]
media_pool = "~/Pictures/Wallpapers"
default_wallpaper = "~/Pictures/Wallpapers/Live/Pokemon/empoleon.mp4"
desktop_notifications = true

[strategy]
mode = "hybrid"
debounce_ms = 250

[compositor]
backend = "hyprland-noctalia"
sync_hyprlock = true
hyprlock_colors_path = "~/.config/hypr/hyprlock-colors.conf"
on_match_exec = "notify-send 'VibeVeil' 'Applied {file} for {title} by {artist}'"

[acoustic]
enabled = true
provider = "lastfm"
fallback = true
lastfm_api_key = "YOUR_LASTFM_API_KEY"
```

### 3.5. Animated Procedural Vinyl Canvas Mode
```toml
[general]
desktop_notifications = true

[strategy]
mode = "vinyl"

[compositor]
backend = "hyprland-noctalia"
```

---

## 4. Interactive Wallpaper Switcher (`vibeveil menu`)

VibeVeil provides an interactive menu command that feeds the indexed media pool directly into Wayland application launchers:

```bash
# Auto-detects rofi, wofi, or dmenu:
vibeveil menu

# Explicit launcher override:
vibeveil menu --launcher "rofi -dmenu -i -p 'Select Wallpaper'"
vibeveil menu --launcher "wofi --dmenu --prompt 'Select Wallpaper'"
```

### Hyprland Keybinding Recipe:
Add to `~/.config/hypr/hyprland.conf`:
```conf
bind = $mainMod, M, exec, vibeveil menu
```

# Waybar & Status Bar Integration

VibeVeil provides native status bar integration via `vibeveil status --json` (or `-j`), emitting standard JSON objects compatible with **Waybar**, **Eww**, and other Wayland status bars.

---

## 1. JSON Payload Specification

```json
{
  "text": "♪ PHNTM - In the Heat of a Disco Night",
  "alt": "playing",
  "tooltip": "Player : Spotify\nStatus : Playing\nTrack  : In the Heat of a Disco Night\nArtist : PHNTM\nAlbum  : In the Heat of a Disco Night\nHold   : Dynamic (Active)\nTheme  : Primary=#ffb599, Accent=#f1dfd9",
  "class": ["playing", "unheld"],
  "player": "spotify",
  "status": "Playing",
  "title": "In the Heat of a Disco Night",
  "artist": "PHNTM",
  "album": "In the Heat of a Disco Night",
  "art_url": "https://i.scdn.co/image/...",
  "held": false
}
```

### JSON Fields

| Field | Type | Description |
| :--- | :--- | :--- |
| `text` | `string` | Display string formatted with status icon, lock indicator, artist, and track title |
| `alt` | `string` | Compact state descriptor: `"playing"`, `"paused"`, `"stopped"`, `"held"`, or `"idle"` |
| `tooltip` | `string` | Multi-line text for bar hovering, including track details, lock status, and active palette |
| `class` | `array<string>` | CSS classes applied to the Waybar widget (e.g. `["playing", "held"]`) |
| `held` | `bool` | `true` if automatic wallpaper switching is locked; `false` otherwise |

---

## 2. Waybar Configuration

Add the module definition to your `~/.config/waybar/config.jsonc`:

```jsonc
"custom/vibeveil": {
    "format": "{}",
    "return-type": "json",
    "interval": 1,
    "exec": "vibeveil status --json",
    "on-click": "vibeveil menu",
    "on-click-right": "vibeveil menu -l 'echo ACTION:toggle_hold'",
    "on-click-middle": "vibeveil menu -l 'echo ACTION:toggle_pause'",
    "tooltip": true,
    "escape": true
}
```

Include `"custom/vibeveil"` in your bar's `"modules-center"` or `"modules-right"` array:

```jsonc
"modules-center": ["custom/vibeveil"],
```

---

## 3. Waybar Styling (CSS)

Add styling rules to your `~/.config/waybar/style.css`:

```css
#custom-vibeveil {
    padding: 0 12px;
    margin: 4px 6px;
    border-radius: 12px;
    background: rgba(26, 17, 14, 0.75);
    color: #f1dfd9;
    font-weight: bold;
    transition: all 0.3s cubic-bezier(0.25, 1, 0.5, 1);
    border: 1px solid rgba(255, 181, 153, 0.2);
}

#custom-vibeveil.playing {
    color: #ffb599;
    border-color: rgba(255, 181, 153, 0.6);
    background: rgba(45, 30, 25, 0.85);
}

#custom-vibeveil.paused {
    color: #a09590;
    opacity: 0.85;
}

#custom-vibeveil.held {
    color: #ffb4a9;
    border-color: #ff5449;
    background: rgba(65, 0, 2, 0.65);
}

#custom-vibeveil.idle {
    color: #605753;
    opacity: 0.6;
}
```

---

## 4. Interactive Actions

The Waybar module exposes three mouse interactions:
- **Left-Click (`on-click`)**: Opens the VibeVeil Rofi/Wofi control menu.
- **Right-Click (`on-click-right`)**: Toggles the wallpaper hold lock (`🔒`), preventing song-driven changes until released.
- **Middle-Click (`on-click-middle`)**: Toggles play/pause state on active MPRIS player.

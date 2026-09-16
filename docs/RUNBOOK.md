# Operational Runbook & Maintenance Guide

| Metadata | Specification |
| :--- | :--- |
| **Project Name** | VibeVeil |
| **Document ID** | OPS-VV-005 |
| **Version** | 1.0.0 |
| **Status** | Approved & Implemented |
| **Audience** | Desktop Operators, System Maintainers & Power Users |
| **Target Service** | `vibeveil.service` (Systemd User Unit) |
| **Primary Authors** | Jose Eduardo Rojas Jimenez |

---

## 1. Quick Operations Reference

| Action | Command | Expected Output / State |
| :--- | :--- | :--- |
| **Initialize Configuration** | `vibeveil init-config` | Generates default `~/.config/vibeveil/config.toml` with reference rulebooks. |
| **Check Active Track & Art** | `vibeveil status` | Prints Spotify player, track, album, status, and CDN art URL. |
| **Dry-Run Track Match** | `vibeveil match` | Tests rulebook & color distance without touching desktop. |
| **Force Immediate Match** | `vibeveil match --apply` | Applies matched wallpaper and repaints window borders immediately. |
| **Interactive Wallpaper Menu**| `vibeveil menu` | Opens Rofi/Wofi/dmenu selector with live media pool for interactive switching. |
| **Shell Autocompletions** | `vibeveil completions bash` | Emits autocompletion scripts for bash, zsh, fish, or powershell. |
| **Re-Index Media Pool** | `vibeveil index` | Scans wallpapers with `indicatif` progress bar and refreshes `pool_index.json`. |
| **Start Daemon Manually** | `vibeveil daemon` | Starts continuous event loop in terminal with verbose logging. |
| **Enable Background Service**| `systemctl --user enable --now vibeveil.service` | Runs daemon as a background systemd user service. |
| **View Daemon Logs** | `journalctl --user -u vibeveil -f` | Real-time stream of track changes and match operations. |

---

## 2. Systemd Service Lifecycle

### 2.1. Service Definition (`~/.config/systemd/user/vibeveil.service`)
```ini
[Unit]
Description=VibeVeil Music-Driven Dynamic Wallpaper Daemon
PartOf=graphical-session.target
After=graphical-session.target

[Service]
Type=simple
ExecStart=/home/eduardo/.local/bin/vibeveil daemon
Restart=always
RestartSec=3
Slice=app.slice

[Install]
WantedBy=graphical-session.target
```

### 2.2. Service Commands
```bash
# Start service
systemctl --user start vibeveil.service

# Stop service
systemctl --user stop vibeveil.service

# Restart service
systemctl --user restart vibeveil.service

# Check health and memory usage
systemctl --user status vibeveil.service
```

---

## 3. Troubleshooting Matrix

| Symptom | Probable Cause | Diagnostic Command | Remediation Protocol |
| :--- | :--- | :--- | :--- |
| `No active MPRIS media player detected` | Spotify is not running, or MPRIS bus is not exposed by sandbox. | `busctl --user list \| grep mpris` | Launch Spotify; if Flatpak, ensure `--filesystem=xdg-run/bus` is granted. |
| Wallpaper changes, but window borders do not update | Hyprland Lua template hook missing `hyprctl reload`. | `hyprctl getoption general:col.active_border` | Verify `~/.config/noctalia/config.toml` includes `post_hook = "/usr/bin/hyprctl reload"`. |
| `No wallpaper match found` | Media pool is empty, unindexed, or `max_acceptable_delta_e` is too strict. | `vibeveil index` | Run `vibeveil index` to scan files; ensure `procedural_fallback = true` in config. |
| High CPU usage during video playback | mpvpaper running in software decoding mode without VA-API. | `journalctl --user -u hypr-livewallpaper -n 30` | Ensure `hypr-livewallpaper.service` uses `hwdec=vaapi vo=gpu` options. |
| Video does not loop or freezes | Broken video codec or corrupt MP4 file. | `mpv --hwdec=vaapi /path/to/video.mp4` | Re-encode to standard H.264 using `ffmpeg -i input -c:v libx264 output.mp4`. |
| Desktop notification not appearing | Notification daemon (mako/dunst/swaync) not running on session D-Bus. | `busctl --user status org.freedesktop.Notifications` | Start or restart user notification daemon (e.g. `systemctl --user restart mako`). |
| Hyprlock lockscreen colors not updated | `sync_hyprlock` disabled or config not sourcing generated file. | `cat ~/.config/hypr/hyprlock-colors.conf` | Ensure `sync_hyprlock = true` in config and `source = ~/.config/hypr/hyprlock-colors.conf` in `hyprlock.conf`. |

---

## 4. Cache & Data Maintenance

VibeVeil stores all volatile state under `$XDG_CACHE_HOME/vibeveil`:

* **Clear Downloaded Album Art:**
  ```bash
  rm -rf ~/.cache/vibeveil/album_art/*
  ```
* **Force Full Pool Re-Index:**
  ```bash
  rm -f ~/.cache/vibeveil/pool_index.json
  vibeveil index
  ```
* **Inspect Current Extracted Colors:**
  ```bash
  cat ~/.config/hypr/noctalia.lua
  ```

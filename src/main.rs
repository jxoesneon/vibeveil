mod acoustic;
mod canvas;
mod color;
mod compositor;
mod config;
mod matcher;
mod mpris;
mod notification;
mod pool;

use anyhow::Result;
use canvas::CanvasGenerator;
use clap::{Parser, Subcommand};
use color::extract_palette_from_image;
use compositor::{CompositorBackend, create_compositor};
use config::{Config, MatchMode, PauseAction};
use matcher::{MatchResult, MatchStrategy, TrackContext, create_matcher};
use mpris::{MprisClient, MprisTrack, PlaybackStatus};
use pool::MediaPool;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Parser, Debug, Clone, PartialEq, Eq)]
#[command(name = "vibeveil")]
#[command(author)]
#[command(version = "0.1.0")]
#[command(about = "Universal music-driven dynamic wallpaper & semantic desktop theming engine", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum Commands {
    /// Start the background Spotify/MPRIS listener daemon
    Daemon {
        /// Optional path to media pool directory
        #[arg(short, long)]
        pool: Option<std::path::PathBuf>,
    },
    /// Match the currently playing track once and print details
    Match {
        /// Apply the matched wallpaper immediately
        #[arg(short, long)]
        apply: bool,
    },
    /// Open an interactive Rofi / Wofi selector menu to manually pick and apply wallpapers
    Menu {
        /// Optional custom launcher command (e.g. rofi, wofi, dmenu)
        #[arg(short, long)]
        launcher: Option<String>,
    },
    /// Re-index and cache color profiles for the media pool
    Index,
    /// Display current MPRIS playback and active theme status
    Status {
        /// Output status formatted as JSON for status bars (Waybar, Eww, etc.)
        #[arg(short, long)]
        json: bool,
    },
    /// Generate a default config file in ~/.config/vibeveil/config.toml
    InitConfig,
    /// Generate shell completions for the specified shell
    Completions {
        /// Shell to generate completions for
        shell: clap_complete::Shell,
    },
}

pub fn execute_init_config(path: &Path, config: &Config) -> Result<String> {
    if path.exists() {
        Ok(format!(
            "Configuration already exists at: {}",
            path.display()
        ))
    } else {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let serialized = toml::to_string_pretty(config)?;
        std::fs::write(path, serialized)?;
        Ok(format!(
            "Initialized configuration template at: {}",
            path.display()
        ))
    }
}

pub fn execute_index_pool(pool: &mut MediaPool, show_progress: bool) -> usize {
    if show_progress {
        let pb = indicatif::ProgressBar::new(0);
        pb.set_style(
            indicatif::ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}")
                .unwrap()
                .progress_chars("#>-"),
        );
        let count = pool.scan(Some(&pb));
        pb.finish_with_message("Done!");
        count
    } else {
        pool.scan(None)
    }
}

pub fn format_status_output(player: Option<&str>, track: Option<&MprisTrack>) -> String {
    if let Some(p) = player {
        let mut s = format!("Active Player : {}\n", p);
        if let Some(t) = track {
            s.push_str(&format!(
                "Track Title   : {}\nArtist        : {}\nAlbum         : {}\nStatus        : {:?}\nArt URL       : {}",
                t.title,
                t.artist,
                t.album,
                t.status,
                t.art_url.as_deref().unwrap_or("None")
            ));
        }
        s
    } else {
        "No active MPRIS media player detected.".into()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StatusJson {
    pub text: String,
    pub alt: String,
    pub tooltip: String,
    pub class: Vec<String>,
    pub player: Option<String>,
    pub status: Option<String>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub art_url: Option<String>,
    pub held: bool,
}

pub fn format_status_json(player: Option<&str>, track: Option<&MprisTrack>, held: bool) -> String {
    let (text, alt, class, status_str) = match (player, track) {
        (Some(_), Some(t)) => {
            let s_str = match t.status {
                PlaybackStatus::Playing => "Playing",
                PlaybackStatus::Paused => "Paused",
                PlaybackStatus::Stopped => "Stopped",
            };
            let symbol = match t.status {
                PlaybackStatus::Playing => "♪",
                PlaybackStatus::Paused => "⏸",
                PlaybackStatus::Stopped => "⏹",
            };
            let lock_prefix = if held { "🔒 " } else { "" };
            let display_text = format!("{}{} {} - {}", lock_prefix, symbol, t.artist, t.title);
            let state_alt = if held {
                "held".to_string()
            } else {
                s_str.to_lowercase()
            };
            let mut classes = vec![s_str.to_lowercase()];
            if held {
                classes.push("held".into());
            } else {
                classes.push("unheld".into());
            }
            (display_text, state_alt, classes, Some(s_str.to_string()))
        }
        (Some(p), None) => {
            let display_text = format!("Connected ({})", p);
            (
                display_text,
                "connected".into(),
                vec!["connected".into()],
                None,
            )
        }
        (None, _) => ("Idle".into(), "idle".into(), vec!["idle".into()], None),
    };

    let theme = load_active_theme();
    let mut tooltip = String::new();
    if let Some(p) = player {
        tooltip.push_str(&format!("Player : {}\n", p));
    }
    if let Some(ref s) = status_str {
        tooltip.push_str(&format!("Status : {}\n", s));
    }
    if let Some(t) = track {
        tooltip.push_str(&format!(
            "Track  : {}\nArtist : {}\nAlbum  : {}\n",
            t.title, t.artist, t.album
        ));
    }
    tooltip.push_str(&format!(
        "Hold   : {}\nTheme  : Primary=#{}, Accent=#{}",
        if held {
            "Locked (Held)"
        } else {
            "Dynamic (Active)"
        },
        theme.accent_hex,
        theme.fg_hex
    ));

    let json_obj = StatusJson {
        text,
        alt,
        tooltip: tooltip.trim_end().to_string(),
        class,
        player: player.map(str::to_string),
        status: status_str,
        title: track.map(|t| t.title.clone()),
        artist: track.map(|t| t.artist.clone()),
        album: track.map(|t| t.album.clone()),
        art_url: track.and_then(|t| t.art_url.clone()),
        held,
    };

    serde_json::to_string(&json_obj).unwrap_or_else(|_| "{}".into())
}

pub fn format_match_result(track: &MprisTrack, matched: &MatchResult) -> String {
    let mut out = format!(
        "=== VibeVeil Match Result ===\nSong      : {} - {}\nStrategy  : {}\nReason    : {}\nConfidence: {:.1}%\nWallpaper : {}",
        track.artist,
        track.title,
        matched.strategy,
        matched.reason,
        matched.score * 100.0,
        matched.wallpaper_path.display()
    );
    if let Some(ref pal) = matched.palette {
        out.push_str(&format!(
            "\nPalette   : Primary={}, Surface={}, Accent={}",
            pal.primary.hex(),
            pal.surface.hex(),
            pal.accent.hex()
        ));
    }
    out
}

pub fn is_wallpaper_held() -> bool {
    dirs::cache_dir()
        .map(|p| p.join("vibeveil/hold_lock").exists())
        .unwrap_or(false)
}

pub fn toggle_wallpaper_hold() -> Result<bool> {
    let path = dirs::cache_dir()
        .map(|p| p.join("vibeveil/hold_lock"))
        .unwrap_or_else(|| std::path::PathBuf::from(".cache/vibeveil/hold_lock"));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    if path.exists() {
        std::fs::remove_file(path)?;
        Ok(false)
    } else {
        std::fs::write(path, b"held")?;
        Ok(true)
    }
}

pub fn handle_playback_status_transition(
    new_status: PlaybackStatus,
    last_status: &mut Option<PlaybackStatus>,
    config: &Config,
    compositor: &dyn CompositorBackend,
    last_track_key: &mut Option<String>,
) {
    if *last_status != Some(new_status) {
        let prev_status = *last_status;
        *last_status = Some(new_status);
        match new_status {
            PlaybackStatus::Paused | PlaybackStatus::Stopped => match config.general.on_pause {
                PauseAction::PausePlayback => {
                    compositor.pause().ok();
                }
                PauseAction::RestoreDefault => {
                    *last_track_key = None;
                    if let Some(ref def) = config.general.default_wallpaper
                        && def.exists()
                    {
                        let is_video = def
                            .extension()
                            .map(|e| e == "mp4" || e == "mkv" || e == "webm")
                            .unwrap_or(false);
                        compositor.apply_wallpaper(def, is_video, None).ok();
                    }
                }
                PauseAction::KeepLast => {}
            },
            PlaybackStatus::Playing => {
                if (prev_status == Some(PlaybackStatus::Paused)
                    || prev_status == Some(PlaybackStatus::Stopped))
                    && config.general.on_pause == PauseAction::RestoreDefault
                {
                    *last_track_key = None;
                }
                compositor.resume().ok();
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn process_track_and_apply(
    track: &MprisTrack,
    pool: &MediaPool,
    matcher: &dyn MatchStrategy,
    compositor: Arc<dyn CompositorBackend>,
    canvas_gen: Arc<CanvasGenerator>,
    apply: bool,
    max_cache_mb: Option<u64>,
    desktop_notifications: bool,
    dbus_connection: Option<&zbus::Connection>,
) -> Option<MatchResult> {
    let mut art_path = None;
    let mut track_palette = None;

    if let Some(ref url) = track.art_url
        && let Ok(cached) = canvas_gen.fetch_or_cache_art(url).await
    {
        track_palette = extract_palette_from_image(&cached);
        art_path = Some(cached.clone());

        if let Some(max_mb) = max_cache_mb
            && let Some(parent) = cached.parent()
        {
            let p = parent.to_path_buf();
            tokio::spawn(async move {
                crate::canvas::prune_lru_cache(&p, max_mb).await;
            });
        }
    }

    let ctx = TrackContext {
        title: track.title.clone(),
        artist: track.artist.clone(),
        album: track.album.clone(),
        art_path,
        palette: track_palette,
    };

    if let Some(matched) = matcher.find_match(&ctx, pool) {
        if apply {
            let _ = compositor.apply_wallpaper_with_meta(
                &matched.wallpaper_path,
                matched.is_video,
                matched.palette.as_ref(),
                Some(&track.title),
                Some(&track.artist),
            );

            // If static poster was applied, asynchronously promote to animated video loop
            if !matched.is_video
                && (matched.strategy == "vinyl-canvas" || matched.strategy == "procedural-canvas")
            {
                let comp = compositor.clone();
                let cg = canvas_gen.clone();
                let art = ctx.art_path.clone();
                let strategy = matched.strategy.clone();
                let pal = matched.palette.clone();
                let title = track.title.clone();
                let artist = track.artist.clone();

                tokio::task::spawn_blocking(move || {
                    if let Some(art_path) = art {
                        let video_res = if strategy == "vinyl-canvas" {
                            cg.generate_vinyl_video_loop(&art_path, 1920, 1080)
                        } else {
                            cg.generate_ambient_video_loop(&art_path, 1920, 1080)
                        };
                        if let Ok(video_path) = video_res {
                            let _ = comp.apply_wallpaper_with_meta(
                                &video_path,
                                true,
                                pal.as_ref(),
                                Some(&title),
                                Some(&artist),
                            );
                        }
                    }
                });
            }

            if desktop_notifications && let Some(conn) = dbus_connection {
                let wall_name = matched
                    .wallpaper_path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Dynamic Canvas")
                    .to_string();
                let title = track.title.clone();
                let artist = track.artist.clone();
                let icon = ctx.art_path.clone();
                let conn = conn.clone();
                tokio::spawn(async move {
                    let _ = notification::send_desktop_notification(
                        &conn,
                        &title,
                        &artist,
                        &wall_name,
                        icon.as_deref(),
                    )
                    .await;
                });
            }
        }
        Some(matched)
    } else {
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemePalette {
    pub bg_hex: String,
    pub fg_hex: String,
    pub accent_hex: String,
    pub selection_bg_hex: String,
    pub selection_fg_hex: String,
}

impl Default for ThemePalette {
    fn default() -> Self {
        Self {
            bg_hex: "1a110e".into(),
            fg_hex: "f1dfd9".into(),
            accent_hex: "ffb599".into(),
            selection_bg_hex: "ffb599".into(),
            selection_fg_hex: "552008".into(),
        }
    }
}

pub fn load_active_theme() -> ThemePalette {
    let mut palette = ThemePalette::default();
    if let Some(home) = dirs::home_dir() {
        let gtk_css = home.join(".config/gtk-3.0/noctalia.css");
        if let Ok(content) = std::fs::read_to_string(&gtk_css) {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("@define-color") {
                    let parts: Vec<&str> = trimmed.split_whitespace().collect();
                    if parts.len() >= 3 {
                        let val = parts[2].trim_matches(';').trim_start_matches('#');
                        if val.len() == 6 {
                            match parts[1] {
                                "accent_color" | "accent_bg_color" => {
                                    palette.accent_hex = val.to_string();
                                    palette.selection_bg_hex = val.to_string();
                                }
                                "window_bg_color" | "view_bg_color" => {
                                    palette.bg_hex = val.to_string();
                                }
                                "window_fg_color" | "view_fg_color" => {
                                    palette.fg_hex = val.to_string();
                                }
                                "accent_fg_color" => {
                                    palette.selection_fg_hex = val.to_string();
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            return palette;
        }

        let kitty_conf = home.join(".config/kitty/themes/noctalia.conf");
        if let Ok(content) = std::fs::read_to_string(&kitty_conf) {
            for line in content.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let val = parts[1].trim_start_matches('#');
                    if val.len() == 6 {
                        match parts[0] {
                            "background" => palette.bg_hex = val.to_string(),
                            "foreground" => palette.fg_hex = val.to_string(),
                            "active_border_color" | "active_tab_background" => {
                                palette.accent_hex = val.to_string();
                                palette.selection_bg_hex = val.to_string();
                            }
                            "active_tab_foreground" => palette.selection_fg_hex = val.to_string(),
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    palette
}

pub async fn run_interactive_menu(launcher: Option<String>, config: &Config) -> Result<()> {
    let mut pool = MediaPool::new(
        config.general.media_pool.clone(),
        config.general.extensions.clone(),
    );
    execute_index_pool(&mut pool, false);
    let items = pool.items();
    if items.is_empty() {
        println!(
            "No wallpapers found in media pool: {}",
            config.general.media_pool.display()
        );
        return Ok(());
    }

    let mpris_client = MprisClient::new().await.ok();
    let active_player = if let Some(ref mpris) = mpris_client {
        mpris.find_active_player().await.ok().flatten()
    } else {
        None
    };

    let active_track = if let (Some(mpris), Some(player)) = (&mpris_client, &active_player) {
        mpris.get_current_track(player).await.ok().flatten()
    } else {
        None
    };

    let mut input_lines = Vec::new();

    if let Some(ref track) = active_track {
        let mood = crate::acoustic::AcousticClassifier::new(config.acoustic.clone())
            .classify_local(&track.title, &track.artist);
        let tag_desc = mood.tags.first().map(|s| s.as_str()).unwrap_or("Dynamic");
        input_lines.push(format!(
            "🎵  Active: {} - {} [{:.0}% Energy • {}]\tACTION:show_status",
            track.title,
            track.artist,
            mood.energy * 100.0,
            tag_desc
        ));
    } else {
        input_lines.push("🎵  No Active Player Detected (Idle)\tACTION:show_status".to_string());
    }

    let mode_label = match config.strategy.mode {
        MatchMode::Vinyl => "Vinyl Record Canvas (60fps Spinning Loop)",
        MatchMode::ProceduralCanvas => "Ambient Blurred Canvas (Animated Motion)",
        MatchMode::Hybrid => "Smart Hybrid (Rulebook + Oklab)",
        MatchMode::Acoustic => "Acoustic Vibe Classifier",
        MatchMode::ColorDistance => "Color Harmony (Oklab)",
        MatchMode::Rulebook => "Rulebook Tag Mapping",
    };

    input_lines.push(format!(
        "✦  Active Daemon Mode: [{}]\tACTION:show_status",
        mode_label
    ));
    input_lines
        .push("──────────────────────────────────────────────────\tACTION:separator".to_string());
    input_lines
        .push("📀  Mode: Spinning Vinyl Record Canvas (60fps)\tACTION:set_mode_vinyl".to_string());
    input_lines
        .push("🖼️  Mode: Animated Ambient Blurred Canvas\tACTION:set_mode_ambient".to_string());
    input_lines.push(
        "🎯  Mode: Smart Hybrid Match (Rulebook + Oklab)\tACTION:set_mode_hybrid".to_string(),
    );
    input_lines.push(
        "🎧  Mode: Acoustic Vibe Classifier (Mood / Energy)\tACTION:set_mode_acoustic".to_string(),
    );
    let held = is_wallpaper_held();
    if held {
        input_lines.push(
            "📌  Wallpaper Lock: ACTIVE (Click to Unlock Auto-Switching)\tACTION:toggle_hold"
                .to_string(),
        );
    } else {
        input_lines.push(
            "📌  Hold Current Wallpaper (Pause Music Reactions)\tACTION:toggle_hold".to_string(),
        );
    }

    let notif_label = if config.general.desktop_notifications {
        "🔔  Desktop Notifications: [ON] (Click to Disable)\tACTION:toggle_notifications"
    } else {
        "🔕  Desktop Notifications: [OFF] (Click to Enable)\tACTION:toggle_notifications"
    };
    input_lines.push(notif_label.to_string());
    input_lines
        .push("🔒  Synchronize Hyprlock Lockscreen Colors\tACTION:sync_hyprlock".to_string());
    input_lines.push(
        "🎨  Resync Dynamic System Theme (Noctalia + Hyprland)\tACTION:resync_theme".to_string(),
    );
    input_lines
        .push("──────────────────────────────────────────────────\tACTION:separator".to_string());
    input_lines.push("⏯  Toggle Playback (Pause / Resume)\tACTION:toggle_pause".to_string());
    input_lines.push("⏭  Next Track\tACTION:next_track".to_string());
    input_lines.push("⏮  Previous Track\tACTION:prev_track".to_string());
    input_lines
        .push("──────────────────────────────────────────────────\tACTION:separator".to_string());
    input_lines.push("🎲  Random Wallpaper (All Categories)\tACTION:random_all".to_string());
    input_lines.push("⚡  Random Pokémon Wallpaper\tACTION:random_pokemon".to_string());
    input_lines.push("🌸  Random Anime Wallpaper\tACTION:random_anime".to_string());
    input_lines
        .push("──────────────────────────────────────────────────\tACTION:separator".to_string());

    for item in &items {
        let path_str = item.path.to_string_lossy();
        let badge = if path_str.contains("/Pokemon/") || path_str.to_lowercase().contains("pokemon")
        {
            "⚡ [Pokémon]"
        } else if path_str.contains("/Anime/") || path_str.to_lowercase().contains("anime") {
            "🌸 [Anime]"
        } else if item.is_video {
            "🎬 [Live]"
        } else {
            "🖼  [Static]"
        };

        let raw_name = item.name.replace(['_', '-'], " ");
        let clean_name = raw_name
            .split_whitespace()
            .map(|word| {
                let mut c = word.chars();
                match c.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ");

        let suffix = if item.is_video {
            " (60fps)"
        } else {
            " (Static)"
        };

        input_lines.push(format!(
            "{:<12} {}{}\t{}",
            badge,
            clean_name,
            suffix,
            item.path.display()
        ));
    }
    let input_str = input_lines.join("\n");

    let (program, args) = match launcher.as_deref() {
        Some(cmd) => {
            let mut parts = cmd.split_whitespace();
            let p = parts.next().unwrap_or("rofi").to_string();
            let a: Vec<String> = parts.map(String::from).collect();
            (p, a)
        }
        None => {
            let theme = load_active_theme();
            if std::process::Command::new("which")
                .arg("rofi")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
            {
                (
                    "rofi".to_string(),
                    vec![
                        "-dmenu".to_string(),
                        "-i".to_string(),
                        "-p".to_string(),
                        "󰋋 VibeVeil".to_string(),
                        "-theme-str".to_string(),
                        format!(
                            "window {{ background-color: #{}; border-color: #{}; border-radius: 16px; }}",
                            theme.bg_hex, theme.accent_hex
                        ),
                    ],
                )
            } else if std::process::Command::new("which")
                .arg("wofi")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
            {
                (
                    "wofi".to_string(),
                    vec![
                        "--dmenu".to_string(),
                        "--prompt".to_string(),
                        "󰋋 VibeVeil".to_string(),
                    ],
                )
            } else if std::process::Command::new("which")
                .arg("fuzzel")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
            {
                (
                    "fuzzel".to_string(),
                    vec![
                        "-d".to_string(),
                        "-p".to_string(),
                        "󰋋 VibeVeil ❯ ".to_string(),
                        "--with-nth=1".to_string(),
                        "--accept-nth=2".to_string(),
                        format!("--background-color={}dd", theme.bg_hex),
                        format!("--text-color={}ff", theme.fg_hex),
                        format!("--prompt-color={}ff", theme.accent_hex),
                        format!("--match-color={}ff", theme.accent_hex),
                        format!("--selection-color={}ee", theme.selection_bg_hex),
                        format!("--selection-text-color={}ff", theme.selection_fg_hex),
                        format!("--selection-match-color={}ff", theme.bg_hex),
                        format!("--border-color={}ff", theme.accent_hex),
                        "--border-width=2".to_string(),
                        "--border-radius=16".to_string(),
                        "--selection-radius=8".to_string(),
                        "--lines=18".to_string(),
                        "--width=58".to_string(),
                        "--horizontal-pad=24".to_string(),
                        "--vertical-pad=16".to_string(),
                        "--inner-pad=8".to_string(),
                        "--line-height=26".to_string(),
                    ],
                )
            } else {
                (
                    "dmenu".to_string(),
                    vec!["-p".to_string(), "VibeVeil Wallpaper".to_string()],
                )
            }
        }
    };

    let mut child = match std::process::Command::new(&program)
        .args(&args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "Warning: could not spawn interactive menu launcher '{}': {}",
                program, e
            );
            return Ok(());
        }
    };

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        let _ = stdin.write_all(input_str.as_bytes());
    }

    let output = match child.wait_with_output() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("Warning: menu launcher '{}' failed to wait: {}", program, e);
            return Ok(());
        }
    };
    if !output.status.success() {
        return Ok(());
    }

    let raw_selected = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if raw_selected.is_empty() || raw_selected.contains("ACTION:separator") {
        return Ok(());
    }

    let action_or_path = if let Some((_, second)) = raw_selected.split_once('\t') {
        second.trim()
    } else if let Some((_, second)) = raw_selected.split_once('|') {
        second.trim()
    } else {
        raw_selected.trim()
    };

    let compositor = create_compositor(&config.compositor);
    let canvas_gen = CanvasGenerator::with_hwaccel(config.strategy.hwaccel);

    match action_or_path {
        "ACTION:show_status" => {
            if let Some(ref track) = active_track {
                let msg = format!(
                    "Player: {}\nTrack: {}\nArtist: {}\nAlbum: {}",
                    track.player, track.title, track.artist, track.album
                );
                let _ = std::process::Command::new("notify-send")
                    .args(["-a", "VibeVeil", "VibeVeil Status", &msg])
                    .status();
            } else {
                let _ = std::process::Command::new("notify-send")
                    .args([
                        "-a",
                        "VibeVeil",
                        "VibeVeil Status",
                        "No active media playback detected.",
                    ])
                    .status();
            }
        }
        "ACTION:set_mode_vinyl" | "ACTION:generate_vinyl" => {
            let mut new_cfg = Config::load();
            new_cfg.strategy.mode = MatchMode::Vinyl;
            let _ = new_cfg.save();

            if let Some(ref track) = active_track
                && let Some(ref art_url) = track.art_url
                && let Ok(art_path) = canvas_gen.fetch_or_cache_art(art_url).await
            {
                println!(
                    "Generating Spinning Vinyl Record Canvas for: {} - {}",
                    track.artist, track.title
                );
                let (canvas_path, is_video) =
                    match canvas_gen.generate_vinyl_video_loop(&art_path, 1920, 1080) {
                        Ok(p) => (p, true),
                        Err(_) => (
                            canvas_gen.generate_vinyl_canvas(&art_path, 1920, 1080)?,
                            false,
                        ),
                    };
                let poster = canvas_path.with_extension("png");
                let palette = extract_palette_from_image(&poster)
                    .or_else(|| extract_palette_from_image(&art_path));
                compositor.apply_wallpaper(&canvas_path, is_video, palette.as_ref())?;
                let _ = std::process::Command::new("notify-send")
                    .args([
                        "-a",
                        "VibeVeil",
                        "-i",
                        art_path.to_string_lossy().as_ref(),
                        "Vinyl Mode Active (60fps)",
                        &format!(
                            "Spinning disc canvas active for {} - {}. Mode persisted.",
                            track.artist, track.title
                        ),
                    ])
                    .status();
            } else {
                let _ = std::process::Command::new("notify-send")
                    .args([
                        "-a",
                        "VibeVeil",
                        "Vinyl Mode Active",
                        "Spinning disc canvas set as active mode for all tracks.",
                    ])
                    .status();
            }
        }
        "ACTION:set_mode_ambient" | "ACTION:generate_ambient" => {
            let mut new_cfg = Config::load();
            new_cfg.strategy.mode = MatchMode::ProceduralCanvas;
            let _ = new_cfg.save();

            if let Some(ref track) = active_track
                && let Some(ref art_url) = track.art_url
                && let Ok(art_path) = canvas_gen.fetch_or_cache_art(art_url).await
            {
                println!(
                    "Generating Animated Ambient Canvas for: {} - {}",
                    track.artist, track.title
                );
                let (canvas_path, is_video) =
                    match canvas_gen.generate_ambient_video_loop(&art_path, 1920, 1080) {
                        Ok(p) => (p, true),
                        Err(_) => (
                            canvas_gen.generate_ambient_canvas(&art_path, 1920, 1080)?,
                            false,
                        ),
                    };
                let poster = canvas_path.with_extension("png");
                let palette = extract_palette_from_image(&poster)
                    .or_else(|| extract_palette_from_image(&art_path));
                compositor.apply_wallpaper(&canvas_path, is_video, palette.as_ref())?;
                let _ = std::process::Command::new("notify-send")
                    .args([
                        "-a",
                        "VibeVeil",
                        "-i",
                        art_path.to_string_lossy().as_ref(),
                        "Ambient Canvas Mode Active",
                        &format!(
                            "Animated ambient backdrop active for {} - {}. Mode persisted.",
                            track.artist, track.title
                        ),
                    ])
                    .status();
            } else {
                let _ = std::process::Command::new("notify-send")
                    .args([
                        "-a",
                        "VibeVeil",
                        "Ambient Canvas Mode Active",
                        "Animated ambient canvas set as active mode for all tracks.",
                    ])
                    .status();
            }
        }
        "ACTION:set_mode_acoustic" | "ACTION:match_acoustic" => {
            let mut new_cfg = Config::load();
            new_cfg.strategy.mode = MatchMode::Acoustic;
            let _ = new_cfg.save();

            if let Some(ref track) = active_track {
                println!(
                    "Evaluating Acoustic Vibe for: {} - {}",
                    track.artist, track.title
                );
                let matcher = crate::matcher::AcousticMatcher::new(&new_cfg);
                let art_path = if let Some(ref url) = track.art_url {
                    canvas_gen.fetch_or_cache_art(url).await.ok()
                } else {
                    None
                };
                let palette = art_path
                    .as_ref()
                    .and_then(|p| extract_palette_from_image(p));
                let ctx = TrackContext {
                    title: track.title.clone(),
                    artist: track.artist.clone(),
                    album: track.album.clone(),
                    art_path,
                    palette,
                };
                if let Some(matched) = matcher.find_match(&ctx, &pool) {
                    compositor.apply_wallpaper(
                        &matched.wallpaper_path,
                        matched.is_video,
                        matched.palette.as_ref(),
                    )?;
                    let _ = std::process::Command::new("notify-send")
                        .args([
                            "-a",
                            "VibeVeil",
                            "Acoustic Match Applied",
                            &format!(
                                "Matched: {} ({})",
                                matched
                                    .wallpaper_path
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy(),
                                matched.reason
                            ),
                        ])
                        .status();
                }
            } else {
                let _ = std::process::Command::new("notify-send")
                    .args([
                        "-a",
                        "VibeVeil",
                        "Acoustic Matcher",
                        "Acoustic mode active. No track currently playing.",
                    ])
                    .status();
            }
        }
        "ACTION:set_mode_hybrid" | "ACTION:match_hybrid" => {
            let mut new_cfg = Config::load();
            new_cfg.strategy.mode = MatchMode::Hybrid;
            let _ = new_cfg.save();

            if let Some(ref track) = active_track {
                println!(
                    "Running Smart Hybrid Match for: {} - {}",
                    track.artist, track.title
                );
                let matcher = create_matcher(&new_cfg);
                let art_path = if let Some(ref url) = track.art_url {
                    canvas_gen.fetch_or_cache_art(url).await.ok()
                } else {
                    None
                };
                let palette = art_path
                    .as_ref()
                    .and_then(|p| extract_palette_from_image(p));
                let ctx = TrackContext {
                    title: track.title.clone(),
                    artist: track.artist.clone(),
                    album: track.album.clone(),
                    art_path,
                    palette,
                };
                if let Some(matched) = matcher.find_match(&ctx, &pool) {
                    compositor.apply_wallpaper(
                        &matched.wallpaper_path,
                        matched.is_video,
                        matched.palette.as_ref(),
                    )?;
                    let _ = std::process::Command::new("notify-send")
                        .args([
                            "-a",
                            "VibeVeil",
                            "Hybrid Mode Active",
                            &format!(
                                "Matched: {} ({:.0}%)",
                                matched
                                    .wallpaper_path
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy(),
                                matched.score * 100.0
                            ),
                        ])
                        .status();
                }
            } else {
                let _ = std::process::Command::new("notify-send")
                    .args([
                        "-a",
                        "VibeVeil",
                        "Hybrid Mode Active",
                        "Smart Hybrid mode set for all tracks.",
                    ])
                    .status();
            }
        }
        "ACTION:toggle_hold" => {
            let now_held = toggle_wallpaper_hold().unwrap_or(false);
            if now_held {
                let _ = std::process::Command::new("notify-send")
                    .args([
                        "-a",
                        "VibeVeil",
                        "Wallpaper Locked",
                        "Automatic wallpaper reactions paused. Current wallpaper held.",
                    ])
                    .status();
            } else {
                let _ = std::process::Command::new("notify-send")
                    .args([
                        "-a",
                        "VibeVeil",
                        "Wallpaper Unlocked",
                        "Dynamic music-driven wallpaper reactions resumed.",
                    ])
                    .status();
            }
        }
        "ACTION:toggle_notifications" => {
            let mut new_cfg = Config::load();
            new_cfg.general.desktop_notifications = !new_cfg.general.desktop_notifications;
            let status_msg = if new_cfg.general.desktop_notifications {
                "Desktop notifications enabled."
            } else {
                "Desktop notifications disabled."
            };
            let _ = new_cfg.save();
            let _ = std::process::Command::new("notify-send")
                .args(["-a", "VibeVeil", "Notifications Toggled", status_msg])
                .status();
        }
        "ACTION:sync_hyprlock" => {
            println!("Synchronizing lockscreen colors...");
            let theme = load_active_theme();
            let parse_rgb = |hex: &str| -> crate::color::ColorRgb {
                let clean = hex.trim_start_matches('#');
                if clean.len() >= 6 {
                    let r = u8::from_str_radix(&clean[0..2], 16).unwrap_or(26);
                    let g = u8::from_str_radix(&clean[2..4], 16).unwrap_or(17);
                    let b = u8::from_str_radix(&clean[4..6], 16).unwrap_or(14);
                    crate::color::ColorRgb::new(r, g, b)
                } else {
                    crate::color::ColorRgb::new(26, 17, 14)
                }
            };
            let default_pal = crate::color::PaletteProfile {
                primary: parse_rgb(&theme.accent_hex),
                secondary: parse_rgb(&theme.fg_hex),
                surface: parse_rgb(&theme.bg_hex),
                accent: parse_rgb(&theme.accent_hex),
            };
            let _ = crate::compositor::sync_hyprlock_palette(
                &config.compositor.hyprlock_colors_path,
                &default_pal,
            );
            let _ = std::process::Command::new("notify-send")
                .args([
                    "-a",
                    "VibeVeil",
                    "Hyprlock Synchronized",
                    &format!(
                        "Palette written to {}",
                        config.compositor.hyprlock_colors_path.display()
                    ),
                ])
                .status();
        }
        "ACTION:resync_theme" => {
            println!("Resyncing theme templates and compositor...");
            let _ = std::process::Command::new("noctalia")
                .args(["msg", "templates-apply"])
                .status();
            let _ = std::process::Command::new("hyprctl").arg("reload").status();
            let _ = std::process::Command::new("notify-send")
                .args([
                    "-a",
                    "VibeVeil",
                    "Theming Resynced",
                    "Applied Noctalia M3 palettes and reloaded Hyprland.",
                ])
                .status();
        }
        "ACTION:toggle_pause" => {
            println!("Toggling playback pause/resume...");
            let _ = std::process::Command::new("noctalia")
                .args(["msg", "media", "toggle"])
                .status()
                .or_else(|_| {
                    std::process::Command::new("playerctl")
                        .arg("play-pause")
                        .status()
                });
        }
        "ACTION:next_track" => {
            let _ = std::process::Command::new("noctalia")
                .args(["msg", "media", "next"])
                .status()
                .or_else(|_| std::process::Command::new("playerctl").arg("next").status());
        }
        "ACTION:prev_track" => {
            let _ = std::process::Command::new("noctalia")
                .args(["msg", "media", "previous"])
                .status()
                .or_else(|_| {
                    std::process::Command::new("playerctl")
                        .arg("previous")
                        .status()
                });
        }
        "ACTION:random_all" => {
            let seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos() as usize;
            let idx = seed % items.len();
            let item = &items[idx];
            println!("Applying random wallpaper: {}", item.path.display());
            let palette = if !item.is_video {
                extract_palette_from_image(&item.path)
            } else {
                None
            };
            compositor.apply_wallpaper(&item.path, item.is_video, palette.as_ref())?;
        }
        "ACTION:random_pokemon" => {
            let candidates: Vec<_> = items
                .iter()
                .filter(|i| i.path.to_string_lossy().to_lowercase().contains("pokemon"))
                .collect();
            let pool_ref = if candidates.is_empty() {
                &items.iter().collect::<Vec<_>>()
            } else {
                &candidates
            };
            let seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos() as usize;
            let idx = seed % pool_ref.len();
            let item = pool_ref[idx];
            println!("Applying random Pokémon wallpaper: {}", item.path.display());
            let palette = if !item.is_video {
                extract_palette_from_image(&item.path)
            } else {
                None
            };
            compositor.apply_wallpaper(&item.path, item.is_video, palette.as_ref())?;
        }
        "ACTION:random_anime" => {
            let candidates: Vec<_> = items
                .iter()
                .filter(|i| i.path.to_string_lossy().to_lowercase().contains("anime"))
                .collect();
            let pool_ref = if candidates.is_empty() {
                &items.iter().collect::<Vec<_>>()
            } else {
                &candidates
            };
            let seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos() as usize;
            let idx = seed % pool_ref.len();
            let item = pool_ref[idx];
            println!("Applying random Anime wallpaper: {}", item.path.display());
            let palette = if !item.is_video {
                extract_palette_from_image(&item.path)
            } else {
                None
            };
            compositor.apply_wallpaper(&item.path, item.is_video, palette.as_ref())?;
        }
        path_str => {
            let selected_path = std::path::PathBuf::from(path_str);
            if selected_path.exists() {
                println!("Applying selected wallpaper: {}", selected_path.display());
                let is_video = selected_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| ["mp4", "mkv", "webm"].contains(&e))
                    .unwrap_or(false);
                let palette = if !is_video {
                    extract_palette_from_image(&selected_path)
                } else {
                    None
                };
                compositor.apply_wallpaper(&selected_path, is_video, palette.as_ref())?;
            } else {
                eprintln!("Selected file does not exist: {}", selected_path.display());
            }
        }
    }

    Ok(())
}

pub fn setup_daemon_components(
    config: &Config,
) -> (
    MediaPool,
    Arc<CanvasGenerator>,
    Arc<dyn MatchStrategy>,
    Arc<dyn CompositorBackend>,
) {
    let pool = MediaPool::new(
        config.general.media_pool.clone(),
        config.general.extensions.clone(),
    );
    let canvas_gen = Arc::new(CanvasGenerator::with_hwaccel(config.strategy.hwaccel));
    let matcher: Arc<dyn MatchStrategy> = Arc::from(create_matcher(config));
    let compositor: Arc<dyn CompositorBackend> = Arc::from(create_compositor(&config.compositor));
    (pool, canvas_gen, matcher, compositor)
}

pub async fn run_cli_command(command: Commands, config: &mut Config) -> Result<()> {
    match command {
        Commands::InitConfig => {
            let path = Config::config_path();
            let msg = execute_init_config(&path, config)?;
            println!("{}", msg);
        }
        Commands::Completions { shell } => {
            use clap::CommandFactory;
            let mut cmd = Cli::command();
            clap_complete::generate(shell, &mut cmd, "vibeveil", &mut std::io::stdout());
        }
        Commands::Index => {
            println!(
                "Indexing media pool: {}",
                config.general.media_pool.display()
            );
            let mut pool = MediaPool::new(
                config.general.media_pool.clone(),
                config.general.extensions.clone(),
            );
            let count = execute_index_pool(&mut pool, true);
            println!("Indexed {} media items with color profiles.", count);
        }
        Commands::Status { json } => {
            let mpris = MprisClient::new().await?;
            let player = mpris.find_active_player().await?;
            let mut track = None;
            if let Some(ref p) = player {
                track = mpris.get_current_track(p).await?;
            }
            if json {
                let held = is_wallpaper_held();
                println!(
                    "{}",
                    format_status_json(player.as_deref(), track.as_ref(), held)
                );
            } else {
                println!(
                    "{}",
                    format_status_output(player.as_deref(), track.as_ref())
                );
            }
        }
        Commands::Menu { launcher } => {
            run_interactive_menu(launcher, config).await?;
        }
        Commands::Match { apply } => {
            let mpris = MprisClient::new().await?;
            let player = match mpris.find_active_player().await? {
                Some(p) => p,
                None => {
                    println!("No active MPRIS player found.");
                    return Ok(());
                }
            };

            let track = match mpris.get_current_track(&player).await? {
                Some(t) => t,
                None => {
                    println!("No track currently playing on player '{}'.", player);
                    return Ok(());
                }
            };

            let mut pool = MediaPool::new(
                config.general.media_pool.clone(),
                config.general.extensions.clone(),
            );
            execute_index_pool(&mut pool, false);

            let canvas_gen = Arc::new(CanvasGenerator::with_hwaccel(config.strategy.hwaccel));
            let matcher = create_matcher(config);
            let compositor: Arc<dyn CompositorBackend> =
                Arc::from(create_compositor(&config.compositor));

            if let Some(matched) = process_track_and_apply(
                &track,
                &pool,
                matcher.as_ref(),
                compositor,
                canvas_gen,
                apply,
                Some(config.general.max_cache_mb),
                config.general.desktop_notifications,
                Some(mpris.connection()),
            )
            .await
            {
                println!("{}", format_match_result(&track, &matched));
                if apply {
                    println!("Applied wallpaper successfully.");
                }
            } else {
                println!(
                    "No wallpaper match found for track: {} - {}",
                    track.artist, track.title
                );
            }
        }
        Commands::Daemon { pool: pool_arg } => {
            if let Some(p) = pool_arg {
                config.general.media_pool = p;
            }

            println!("✦ VibeVeil Daemon Initializing...");
            println!("  Media Pool : {}", config.general.media_pool.display());
            println!("  Match Mode : {:?}", config.strategy.mode);
            println!("  Compositor : {:?}", config.compositor.backend);

            let (mut pool, canvas_gen, _matcher, compositor) = setup_daemon_components(config);
            let count = execute_index_pool(&mut pool, false);
            println!("  Indexed    : {} items in pool", count);

            let mpris = Arc::new(MprisClient::new().await?);
            let pool = Arc::new(pool);
            let _max_cache_mb = config.general.max_cache_mb;

            let mut last_track_key: Option<String> = None;
            let mut last_status: Option<PlaybackStatus> = None;

            println!("✦ Listening for Spotify/MPRIS audio streams (Zero-CPU event loop active)...");

            let mut abort_handle: Option<tokio::task::AbortHandle> = None;

            use futures_util::StreamExt;

            'reconnect_loop: loop {
                let mut player = None;
                while player.is_none() {
                    player = match mpris.find_active_player().await {
                        Ok(p) => p,
                        Err(e) => {
                            eprintln!("Warning: Error querying MPRIS players: {e}");
                            None
                        }
                    };
                    if player.is_none() {
                        sleep(Duration::from_millis(2000)).await;
                    }
                }
                let player = player.unwrap();
                println!("Connected to player: {}", player);

                let mut stream = match mpris.listen_for_properties_changed(&player).await {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!(
                            "Warning: Failed to subscribe to player properties: {e}. Retrying in 2s..."
                        );
                        sleep(Duration::from_millis(2000)).await;
                        continue 'reconnect_loop;
                    }
                };

                loop {
                    tokio::select! {
                        msg = stream.next() => {
                            if msg.is_none() {
                                println!("⚡ Player disconnected. Re-scanning for active MPRIS players...");
                                break;
                            }

                            if let Ok(Some(track)) = mpris.get_current_track(&player).await {
                                let track_key = format!("{}:{}:{}", track.artist, track.title, track.album);
                                let active_cfg = Config::load();

                                handle_playback_status_transition(
                                    track.status,
                                    &mut last_status,
                                    &active_cfg,
                                    compositor.as_ref(),
                                    &mut last_track_key,
                                );

                                if is_wallpaper_held() {
                                    continue;
                                }

                                if track.status == PlaybackStatus::Playing && last_track_key.as_deref() != Some(&track_key) {
                                    last_track_key = Some(track_key.clone());
                                    println!("▶ Now Playing: {} — {}", track.artist, track.title);

                                    if let Some(ah) = abort_handle.take() {
                                        ah.abort();
                                    }

                                    let pool_clone = pool.clone();
                                    let canvas_gen_clone = canvas_gen.clone();
                                    let dynamic_matcher: Arc<dyn MatchStrategy> = Arc::from(create_matcher(&active_cfg));
                                    let compositor_clone = compositor.clone();
                                    let dbus_conn = mpris.connection().clone();
                                    let notifications = active_cfg.general.desktop_notifications;
                                    let cache_limit = active_cfg.general.max_cache_mb;

                                    let handle = tokio::spawn(async move {
                                        if let Some(matched) = process_track_and_apply(
                                            &track,
                                            &pool_clone,
                                            dynamic_matcher.as_ref(),
                                            compositor_clone,
                                            canvas_gen_clone,
                                            true,
                                            Some(cache_limit),
                                            notifications,
                                            Some(&dbus_conn),
                                        ).await {
                                            println!("  ↳ Matched: {} via {} ({})", matched.wallpaper_path.display(), matched.strategy, matched.reason);
                                        }
                                    });
                                    abort_handle = Some(handle.abort_handle());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut config = Config::load();
    run_cli_command(cli.command, &mut config).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::{ColorRgb, PaletteProfile};
    use tempfile::TempDir;

    #[test]
    fn cli_parse_subcommands() {
        let c1 = Cli::try_parse_from(["vibeveil", "init-config"]).unwrap();
        assert_eq!(c1.command, Commands::InitConfig);

        let c2 = Cli::try_parse_from(["vibeveil", "index"]).unwrap();
        assert_eq!(c2.command, Commands::Index);

        let c3 = Cli::try_parse_from(["vibeveil", "status"]).unwrap();
        assert_eq!(c3.command, Commands::Status { json: false });

        let c3_json = Cli::try_parse_from(["vibeveil", "status", "--json"]).unwrap();
        assert_eq!(c3_json.command, Commands::Status { json: true });

        let c3_j = Cli::try_parse_from(["vibeveil", "status", "-j"]).unwrap();
        assert_eq!(c3_j.command, Commands::Status { json: true });

        let c4 = Cli::try_parse_from(["vibeveil", "match"]).unwrap();
        assert_eq!(c4.command, Commands::Match { apply: false });

        let c5 = Cli::try_parse_from(["vibeveil", "match", "--apply"]).unwrap();
        assert_eq!(c5.command, Commands::Match { apply: true });

        let c6 = Cli::try_parse_from(["vibeveil", "match", "-a"]).unwrap();
        assert_eq!(c6.command, Commands::Match { apply: true });

        let c7 = Cli::try_parse_from(["vibeveil", "daemon"]).unwrap();
        assert_eq!(c7.command, Commands::Daemon { pool: None });

        let c8 = Cli::try_parse_from(["vibeveil", "daemon", "--pool", "/my/pool"]).unwrap();
        assert_eq!(
            c8.command,
            Commands::Daemon {
                pool: Some(std::path::PathBuf::from("/my/pool"))
            }
        );

        let c9 = Cli::try_parse_from(["vibeveil", "daemon", "-p", "/my/pool"]).unwrap();
        assert_eq!(
            c9.command,
            Commands::Daemon {
                pool: Some(std::path::PathBuf::from("/my/pool"))
            }
        );

        let err = Cli::try_parse_from(["vibeveil", "nonexistent-command"]);
        assert!(err.is_err());
    }

    #[test]
    fn execute_init_config_creates_new_file() {
        let dir = TempDir::new().unwrap();
        let target = dir.path().join("sub/config.toml");
        let cfg = Config::default();

        let msg = execute_init_config(&target, &cfg).unwrap();
        assert!(msg.contains("Initialized configuration template"));
        assert!(target.exists());

        // Calling again indicates it already exists
        let msg2 = execute_init_config(&target, &cfg).unwrap();
        assert!(msg2.contains("Configuration already exists"));
    }

    #[test]
    fn execute_index_pool_runs() {
        let dir = TempDir::new().unwrap();
        let cache_file = dir.path().join("idx.json");
        let mut pool =
            MediaPool::new_isolated(dir.path().to_path_buf(), vec!["png".into()], cache_file);

        // With and without progress bar
        let c1 = execute_index_pool(&mut pool, false);
        assert_eq!(c1, 0);

        let c2 = execute_index_pool(&mut pool, true);
        assert_eq!(c2, 0);
    }

    #[test]
    fn format_status_output_variations() {
        let s_none = format_status_output(None, None);
        assert!(s_none.contains("No active MPRIS"));

        let s_player_only = format_status_output(Some("spotify"), None);
        assert!(s_player_only.contains("Active Player : spotify"));

        let track = MprisTrack {
            player: "spotify".into(),
            title: "Track1".into(),
            artist: "Artist1".into(),
            album: "Album1".into(),
            art_url: Some("https://example.com/art.png".into()),
            status: PlaybackStatus::Playing,
        };
        let s_full = format_status_output(Some("spotify"), Some(&track));
        assert!(s_full.contains("Track Title   : Track1"));
        assert!(s_full.contains("Status        : Playing"));
        assert!(s_full.contains("Art URL       : https://example.com/art.png"));
    }

    #[test]
    fn format_status_json_variations() {
        let j_none = format_status_json(None, None, false);
        let parsed_none: StatusJson = serde_json::from_str(&j_none).unwrap();
        assert_eq!(parsed_none.alt, "idle");
        assert_eq!(parsed_none.class, vec!["idle"]);
        assert!(!parsed_none.held);

        let j_player = format_status_json(Some("spotify"), None, false);
        let parsed_player: StatusJson = serde_json::from_str(&j_player).unwrap();
        assert_eq!(parsed_player.alt, "connected");
        assert_eq!(parsed_player.player.as_deref(), Some("spotify"));

        let track = MprisTrack {
            player: "spotify".into(),
            title: "Track1".into(),
            artist: "Artist1".into(),
            album: "Album1".into(),
            art_url: Some("https://example.com/art.png".into()),
            status: PlaybackStatus::Playing,
        };
        let j_full = format_status_json(Some("spotify"), Some(&track), false);
        let parsed_full: StatusJson = serde_json::from_str(&j_full).unwrap();
        assert_eq!(parsed_full.alt, "playing");
        assert!(parsed_full.class.contains(&"playing".to_string()));
        assert!(parsed_full.class.contains(&"unheld".to_string()));
        assert_eq!(parsed_full.title.as_deref(), Some("Track1"));
        assert!(!parsed_full.held);

        let j_held = format_status_json(Some("spotify"), Some(&track), true);
        let parsed_held: StatusJson = serde_json::from_str(&j_held).unwrap();
        assert_eq!(parsed_held.alt, "held");
        assert!(parsed_held.class.contains(&"held".to_string()));
        assert!(parsed_held.held);
    }

    #[test]
    fn format_match_result_formatting() {
        let track = MprisTrack {
            player: "spotify".into(),
            title: "Starboy".into(),
            artist: "The Weeknd".into(),
            album: "Starboy".into(),
            art_url: None,
            status: PlaybackStatus::Playing,
        };
        let matched = MatchResult {
            wallpaper_path: std::path::PathBuf::from("/wallpapers/space.png"),
            is_video: false,
            palette: Some(PaletteProfile {
                primary: ColorRgb::new(255, 0, 0),
                secondary: ColorRgb::new(0, 255, 0),
                surface: ColorRgb::new(20, 20, 20),
                accent: ColorRgb::new(0, 0, 255),
            }),
            score: 0.95,
            strategy: "rulebook".into(),
            reason: "Genre match".into(),
        };

        let out = format_match_result(&track, &matched);
        assert!(out.contains("Song      : The Weeknd - Starboy"));
        assert!(out.contains("Confidence: 95.0%"));
        assert!(out.contains("Palette   : Primary=#ff0000"));
    }

    #[test]
    fn handle_playback_status_transition_branches() {
        let compositor = compositor::CustomCommandBackend::new("echo {file}".into());
        let mut last_status = None;
        let mut last_track_key = Some("Artist:Title:Album".to_string());
        let mut cfg = Config::default();

        // 1. Transition to Playing
        handle_playback_status_transition(
            PlaybackStatus::Playing,
            &mut last_status,
            &cfg,
            &compositor,
            &mut last_track_key,
        );
        assert_eq!(last_status, Some(PlaybackStatus::Playing));
        assert_eq!(last_track_key.as_deref(), Some("Artist:Title:Album"));

        // 2. Same status -> no-op
        handle_playback_status_transition(
            PlaybackStatus::Playing,
            &mut last_status,
            &cfg,
            &compositor,
            &mut last_track_key,
        );
        assert_eq!(last_status, Some(PlaybackStatus::Playing));

        // 3. Transition to Paused with PausePlayback
        cfg.general.on_pause = PauseAction::PausePlayback;
        handle_playback_status_transition(
            PlaybackStatus::Paused,
            &mut last_status,
            &cfg,
            &compositor,
            &mut last_track_key,
        );
        assert_eq!(last_status, Some(PlaybackStatus::Paused));

        // 4. Transition to Stopped with KeepLast
        cfg.general.on_pause = PauseAction::KeepLast;
        handle_playback_status_transition(
            PlaybackStatus::Stopped,
            &mut last_status,
            &cfg,
            &compositor,
            &mut last_track_key,
        );
        assert_eq!(last_status, Some(PlaybackStatus::Stopped));

        // 5. Transition to Paused with RestoreDefault resets last_track_key
        cfg.general.on_pause = PauseAction::RestoreDefault;
        cfg.general.default_wallpaper = None;
        handle_playback_status_transition(
            PlaybackStatus::Paused,
            &mut last_status,
            &cfg,
            &compositor,
            &mut last_track_key,
        );
        assert_eq!(last_status, Some(PlaybackStatus::Paused));
        assert_eq!(last_track_key, None);
    }

    #[tokio::test]
    async fn process_track_and_apply_flow() {
        let dir = TempDir::new().unwrap();
        let art_path = dir.path().join("art.png");
        let img = image::RgbImage::from_fn(16, 16, |_, _| image::Rgb([180, 40, 40]));
        img.save(&art_path).unwrap();

        let wall_path = dir.path().join("wall.png");
        let wall_img = image::RgbImage::from_fn(16, 16, |_, _| image::Rgb([180, 40, 40]));
        wall_img.save(&wall_path).unwrap();

        let cache_file = dir.path().join("idx.json");
        let mut pool =
            MediaPool::new_isolated(dir.path().to_path_buf(), vec!["png".into()], cache_file);
        pool.scan(None);

        let track = MprisTrack {
            player: "spotify".into(),
            title: "Rock Anthem".into(),
            artist: "Metal".into(),
            album: "Doom".into(),
            art_url: Some(format!("file://{}", art_path.display())),
            status: PlaybackStatus::Playing,
        };

        let cfg = Config {
            rules: vec![crate::config::RuleConfig {
                match_field: crate::config::MatchField::Any,
                pattern: "(?i)metal".into(),
                target_tag_or_path: "wall".into(),
            }],
            ..Config::default()
        };

        let matcher = create_matcher(&cfg);
        let compositor: Arc<dyn CompositorBackend> =
            Arc::new(compositor::CustomCommandBackend::new("echo {file}".into()));
        let canvas_gen = Arc::new(CanvasGenerator::new());

        let res = process_track_and_apply(
            &track,
            &pool,
            matcher.as_ref(),
            compositor,
            canvas_gen,
            true,
            Some(100),
            false,
            None,
        )
        .await;

        assert!(res.is_some());
        let m = res.unwrap();
        assert_eq!(m.strategy, "rulebook");
    }

    #[test]
    fn test_setup_daemon_components() {
        let cfg = Config::default();
        let (_pool, _cg, matcher, compositor) = setup_daemon_components(&cfg);
        assert_eq!(matcher.name(), "hybrid");
        assert_eq!(compositor.name(), "hyprland-noctalia");
    }

    #[tokio::test]
    async fn test_run_cli_command_index_and_init() {
        let dir = TempDir::new().unwrap();
        let mut cfg = Config::default();
        cfg.general.media_pool = dir.path().to_path_buf();

        // Test Index command
        let res_idx = run_cli_command(Commands::Index, &mut cfg).await;
        assert!(res_idx.is_ok());

        // Test Status command (tolerates live player or absence of player)
        let _ = run_cli_command(Commands::Status { json: false }, &mut cfg).await;
        let _ = run_cli_command(Commands::Status { json: true }, &mut cfg).await;

        // Test Match command
        let _ = run_cli_command(Commands::Match { apply: false }, &mut cfg).await;

        // Test Menu command on empty pool
        let res_menu = run_cli_command(Commands::Menu { launcher: None }, &mut cfg).await;
        assert!(res_menu.is_ok());

        // Test Completions command
        let res_comp = run_cli_command(
            Commands::Completions {
                shell: clap_complete::Shell::Bash,
            },
            &mut cfg,
        )
        .await;
        assert!(res_comp.is_ok());

        // Test Daemon command initiation with a timeout
        let daemon_dir = TempDir::new().unwrap();
        let _ = tokio::time::timeout(
            Duration::from_millis(50),
            run_cli_command(
                Commands::Daemon {
                    pool: Some(daemon_dir.path().to_path_buf()),
                },
                &mut cfg,
            ),
        )
        .await;
    }

    #[test]
    fn test_theme_palette_defaults_and_active_loader() {
        let default_palette = ThemePalette::default();
        assert_eq!(default_palette.bg_hex, "1a110e");
        assert_eq!(default_palette.fg_hex, "f1dfd9");
        assert_eq!(default_palette.accent_hex, "ffb599");

        let active_palette = load_active_theme();
        assert_eq!(active_palette.bg_hex.len(), 6);
        assert_eq!(active_palette.fg_hex.len(), 6);
        assert_eq!(active_palette.accent_hex.len(), 6);
    }

    #[tokio::test]
    async fn test_wallpaper_hold_toggle_logic() {
        struct HoldLockRestoreGuard {
            original_held: bool,
        }
        impl Drop for HoldLockRestoreGuard {
            fn drop(&mut self) {
                if is_wallpaper_held() != self.original_held {
                    let _ = toggle_wallpaper_hold();
                }
            }
        }
        let _guard = HoldLockRestoreGuard {
            original_held: is_wallpaper_held(),
        };

        let original = is_wallpaper_held();
        let toggled = toggle_wallpaper_hold().unwrap();
        assert_eq!(toggled, !original);
        assert_eq!(is_wallpaper_held(), !original);
        let restored = toggle_wallpaper_hold().unwrap();
        assert_eq!(restored, original);
        assert_eq!(is_wallpaper_held(), original);
    }

    #[tokio::test]
    async fn test_interactive_menu_all_action_branches() {
        struct HoldLockRestoreGuard {
            original_held: bool,
        }
        impl Drop for HoldLockRestoreGuard {
            fn drop(&mut self) {
                if is_wallpaper_held() != self.original_held {
                    let _ = toggle_wallpaper_hold();
                }
            }
        }
        let _hold_guard = HoldLockRestoreGuard {
            original_held: is_wallpaper_held(),
        };

        struct ConfigRestoreGuard {
            path: std::path::PathBuf,
            original_content: Option<String>,
        }
        impl Drop for ConfigRestoreGuard {
            fn drop(&mut self) {
                if let Some(ref content) = self.original_content {
                    let _ = std::fs::write(&self.path, content);
                } else {
                    let _ = std::fs::remove_file(&self.path);
                }
            }
        }
        let cfg_path = Config::config_path();
        let _guard = ConfigRestoreGuard {
            original_content: std::fs::read_to_string(&cfg_path).ok(),
            path: cfg_path,
        };

        let pool_dir = TempDir::new().unwrap();
        let test_img = pool_dir.path().join("pokemon_lucario.png");
        let img = image::RgbImage::from_fn(32, 32, |_, _| image::Rgb([10, 20, 30]));
        img.save(&test_img).unwrap();

        let test_vid = pool_dir.path().join("anime_cyberpunk.mp4");
        std::fs::write(&test_vid, b"fake_mp4_bytes").unwrap();

        let lock_file = pool_dir.path().join("hyprlock.conf");

        let mut cfg = Config::default();
        cfg.general.media_pool = pool_dir.path().to_path_buf();
        cfg.compositor.backend = config::BackendType::Command;
        cfg.compositor.custom_command = Some("echo {file}".into());
        cfg.compositor.hyprlock_colors_path = lock_file;

        let actions = [
            "ACTION:show_status",
            "ACTION:toggle_hold",
            "ACTION:toggle_notifications",
            "ACTION:sync_hyprlock",
            "ACTION:resync_theme",
            "ACTION:toggle_pause",
            "ACTION:next_track",
            "ACTION:prev_track",
            "ACTION:random_all",
            "ACTION:random_pokemon",
            "ACTION:random_anime",
            "ACTION:set_mode_hybrid",
            "ACTION:set_mode_ambient",
            "ACTION:set_mode_vinyl",
            "ACTION:set_mode_acoustic",
            "ACTION:set_mode_color",
            "ACTION:set_mode_rulebook",
            "ACTION:separator",
        ];

        for action in actions {
            let cmd = format!("echo {}", action);
            let res = run_interactive_menu(Some(cmd), &cfg).await;
            assert!(res.is_ok());
        }

        // Test direct file selection
        let file_cmd = format!("echo {}", test_img.display());
        let res_file = run_interactive_menu(Some(file_cmd), &cfg).await;
        assert!(res_file.is_ok());

        // Test video file selection
        let vid_cmd = format!("echo {}", test_vid.display());
        let res_vid = run_interactive_menu(Some(vid_cmd), &cfg).await;
        assert!(res_vid.is_ok());

        // Test empty selection
        let empty_cmd = "echo".to_string();
        let res_empty = run_interactive_menu(Some(empty_cmd), &cfg).await;
        assert!(res_empty.is_ok());
    }
}

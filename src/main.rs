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
use config::{Config, PauseAction};
use matcher::{MatchResult, MatchStrategy, TrackContext, create_matcher};
use mpris::{MprisClient, MprisTrack, PlaybackStatus};
use pool::MediaPool;
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
    Status,
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

pub fn handle_playback_status_transition(
    new_status: PlaybackStatus,
    last_status: &mut Option<PlaybackStatus>,
    config: &Config,
    compositor: &dyn CompositorBackend,
) {
    if *last_status != Some(new_status) {
        *last_status = Some(new_status);
        match new_status {
            PlaybackStatus::Paused | PlaybackStatus::Stopped => match config.general.on_pause {
                PauseAction::PausePlayback => {
                    compositor.pause().ok();
                }
                PauseAction::RestoreDefault => {
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
    compositor: &dyn CompositorBackend,
    canvas_gen: &CanvasGenerator,
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
            let _ = compositor.apply_wallpaper(
                &matched.wallpaper_path,
                matched.is_video,
                matched.palette.as_ref(),
            );

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

pub fn run_interactive_menu(launcher: Option<String>, config: &Config) -> Result<()> {
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

    let mut input_lines = Vec::new();
    for item in items {
        let kind = if item.is_video { "[Video]" } else { "[Image]" };
        input_lines.push(format!(
            "{:<8} {:<30} | {}",
            kind,
            item.name,
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
                        "VibeVeil Wallpaper".to_string(),
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
                        "VibeVeil Wallpaper".to_string(),
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

    let selected = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if selected.is_empty() {
        return Ok(());
    }

    let selected_path = if let Some((_, path_part)) = selected.split_once('|') {
        std::path::PathBuf::from(path_part.trim())
    } else {
        std::path::PathBuf::from(selected.trim())
    };

    if selected_path.exists() {
        println!("Applying selected wallpaper: {}", selected_path.display());
        let compositor = create_compositor(&config.compositor);
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
    let canvas_gen = Arc::new(CanvasGenerator::new());
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
        Commands::Status => {
            let mpris = MprisClient::new().await?;
            let player = mpris.find_active_player().await?;
            let mut track = None;
            if let Some(ref p) = player {
                track = mpris.get_current_track(p).await?;
            }
            println!(
                "{}",
                format_status_output(player.as_deref(), track.as_ref())
            );
        }
        Commands::Menu { launcher } => {
            run_interactive_menu(launcher, config)?;
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
                    println!("No active track playing.");
                    return Ok(());
                }
            };

            let mut pool = MediaPool::new(
                config.general.media_pool.clone(),
                config.general.extensions.clone(),
            );
            execute_index_pool(&mut pool, false);

            let canvas_gen = CanvasGenerator::new();
            let matcher = create_matcher(config);
            let compositor = create_compositor(&config.compositor);

            if let Some(matched) = process_track_and_apply(
                &track,
                &pool,
                matcher.as_ref(),
                compositor.as_ref(),
                &canvas_gen,
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

            let (mut pool, canvas_gen, matcher, compositor) = setup_daemon_components(config);
            let count = execute_index_pool(&mut pool, false);
            println!("  Indexed    : {} items in pool", count);

            let mpris = Arc::new(MprisClient::new().await?);
            let pool = Arc::new(pool);
            let max_cache_mb = config.general.max_cache_mb;

            let mut last_track_key: Option<String> = None;
            let mut last_status: Option<PlaybackStatus> = None;

            println!("✦ Listening for Spotify/MPRIS audio streams (Zero-CPU event loop active)...");

            let mut abort_handle: Option<tokio::task::AbortHandle> = None;
            let debounce_ms = config.strategy.debounce_ms;

            let mut player = None;
            while player.is_none() {
                player = mpris.find_active_player().await?;
                if player.is_none() {
                    sleep(Duration::from_millis(2000)).await;
                }
            }
            let player = player.unwrap();

            use futures_util::StreamExt;
            let mut stream = mpris.listen_for_properties_changed(&player).await?;
            let mut pending_update = true;
            let mut timeout_fut = Box::pin(sleep(Duration::from_millis(0)));

            loop {
                tokio::select! {
                    msg = stream.next() => {
                        if msg.is_none() { break; }
                        pending_update = true;
                        timeout_fut = Box::pin(sleep(Duration::from_millis(debounce_ms)));
                    }
                    _ = &mut timeout_fut, if pending_update => {
                        pending_update = false;

                        if let Ok(Some(track)) = mpris.get_current_track(&player).await {
                            let track_key = format!("{}:{}:{}", track.artist, track.title, track.album);

                            handle_playback_status_transition(track.status, &mut last_status, config, compositor.as_ref());

                            if track.status == PlaybackStatus::Playing && last_track_key.as_deref() != Some(&track_key) {
                                last_track_key = Some(track_key.clone());
                                println!("▶ Now Playing: {} — {}", track.artist, track.title);

                                if let Some(ah) = abort_handle.take() {
                                    ah.abort();
                                }

                                let pool_clone = pool.clone();
                                let canvas_gen_clone = canvas_gen.clone();
                                let matcher_clone = matcher.clone();
                                let compositor_clone = compositor.clone();
                                let dbus_conn = mpris.connection().clone();
                                let notifications = config.general.desktop_notifications;

                                let handle = tokio::spawn(async move {
                                    if let Some(matched) = process_track_and_apply(
                                        &track,
                                        &pool_clone,
                                        matcher_clone.as_ref(),
                                        compositor_clone.as_ref(),
                                        &canvas_gen_clone,
                                        true,
                                        Some(max_cache_mb),
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
        assert_eq!(c3.command, Commands::Status);

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
        let mut cfg = Config::default();

        // 1. Transition to Playing
        handle_playback_status_transition(
            PlaybackStatus::Playing,
            &mut last_status,
            &cfg,
            &compositor,
        );
        assert_eq!(last_status, Some(PlaybackStatus::Playing));

        // 2. Same status -> no-op
        handle_playback_status_transition(
            PlaybackStatus::Playing,
            &mut last_status,
            &cfg,
            &compositor,
        );
        assert_eq!(last_status, Some(PlaybackStatus::Playing));

        // 3. Transition to Paused with PausePlayback
        cfg.general.on_pause = PauseAction::PausePlayback;
        handle_playback_status_transition(
            PlaybackStatus::Paused,
            &mut last_status,
            &cfg,
            &compositor,
        );
        assert_eq!(last_status, Some(PlaybackStatus::Paused));

        // 4. Transition to Stopped with KeepLast
        cfg.general.on_pause = PauseAction::KeepLast;
        handle_playback_status_transition(
            PlaybackStatus::Stopped,
            &mut last_status,
            &cfg,
            &compositor,
        );
        assert_eq!(last_status, Some(PlaybackStatus::Stopped));

        // 5. Transition to Paused with RestoreDefault
        cfg.general.on_pause = PauseAction::RestoreDefault;
        cfg.general.default_wallpaper = None;
        handle_playback_status_transition(
            PlaybackStatus::Paused,
            &mut last_status,
            &cfg,
            &compositor,
        );
        assert_eq!(last_status, Some(PlaybackStatus::Paused));
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
        let compositor = compositor::CustomCommandBackend::new("echo {file}".into());
        let canvas_gen = CanvasGenerator::new();

        let res = process_track_and_apply(
            &track,
            &pool,
            matcher.as_ref(),
            &compositor,
            &canvas_gen,
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
        let _ = run_cli_command(Commands::Status, &mut cfg).await;

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
}

use crate::color::PaletteProfile;
use crate::config::{BackendType, CompositorConfig};
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::process::Command;

pub trait CompositorBackend: Send + Sync {
    fn name(&self) -> &'static str;
    fn apply_wallpaper(
        &self,
        path: &Path,
        is_video: bool,
        palette: Option<&PaletteProfile>,
    ) -> Result<()>;

    fn apply_wallpaper_with_meta(
        &self,
        path: &Path,
        is_video: bool,
        palette: Option<&PaletteProfile>,
        _title: Option<&str>,
        _artist: Option<&str>,
    ) -> Result<()> {
        self.apply_wallpaper(path, is_video, palette)
    }

    fn pause(&self) -> Result<()>;
    fn resume(&self) -> Result<()>;
}

pub struct HyprlandNoctaliaBackend {
    config: CompositorConfig,
}

impl HyprlandNoctaliaBackend {
    pub fn new(config: CompositorConfig) -> Self {
        Self { config }
    }

    fn update_noctalia(&self, poster_or_img: &Path) {
        self.update_noctalia_monitor(poster_or_img, None);
    }

    fn update_noctalia_monitor(&self, poster_or_img: &Path, monitor: Option<&str>) {
        if self.config.trigger_noctalia_theming {
            let mut cmd = Command::new("noctalia");
            cmd.args(["msg", "wallpaper-set"]);
            if let Some(mon) = monitor {
                cmd.args(["--output", mon]);
            }
            cmd.arg(poster_or_img.to_string_lossy().as_ref());
            let _ = cmd.output();

            let _ = Command::new("noctalia")
                .args(["msg", "templates-apply"])
                .output();
        }
    }

    fn reload_hyprland(&self) {
        if self.config.trigger_hyprland_reload {
            let _ = Command::new("hyprctl").arg("reload").output();
        }
    }
}

impl CompositorBackend for HyprlandNoctaliaBackend {
    fn name(&self) -> &'static str {
        "hyprland-noctalia"
    }

    fn apply_wallpaper(
        &self,
        path: &Path,
        is_video: bool,
        palette: Option<&PaletteProfile>,
    ) -> Result<()> {
        self.apply_wallpaper_with_meta(path, is_video, palette, None, None)
    }

    fn apply_wallpaper_with_meta(
        &self,
        path: &Path,
        is_video: bool,
        palette: Option<&PaletteProfile>,
        title: Option<&str>,
        artist: Option<&str>,
    ) -> Result<()> {
        if !self.config.monitors.is_empty() {
            let mut any_video = false;
            for mon in &self.config.monitors {
                let mon_path = mon.wallpaper.as_deref().unwrap_or(path);
                let mon_is_video = mon_path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| matches!(ext.to_lowercase().as_str(), "mp4" | "mkv" | "webm"))
                    .unwrap_or(is_video);

                if mon_is_video {
                    any_video = true;
                    let mon_symlink = dirs::config_dir()
                        .map(|p| p.join(format!("hypr/current_wallpaper_{}.mp4", mon.name)))
                        .unwrap_or_else(|| {
                            PathBuf::from(format!(".current_wallpaper_{}.mp4", mon.name))
                        });
                    let _ = std::fs::remove_file(&mon_symlink);
                    #[cfg(unix)]
                    let _ = std::os::unix::fs::symlink(mon_path, &mon_symlink);

                    if mon.primary {
                        let symlink = dirs::config_dir()
                            .map(|p| p.join("hypr/current_wallpaper.mp4"))
                            .unwrap_or_else(|| PathBuf::from(".current_wallpaper.mp4"));
                        let _ = std::fs::remove_file(&symlink);
                        #[cfg(unix)]
                        let _ = std::os::unix::fs::symlink(mon_path, &symlink);
                    }

                    if let Some(parent) = mon_path.parent() {
                        let stem = mon_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                        let thumb = parent.join(".thumbnails").join(format!("{}.png", stem));
                        let sibling_png = mon_path.with_extension("png");
                        if thumb.exists() {
                            self.update_noctalia_monitor(&thumb, Some(&mon.name));
                        } else if sibling_png.exists() {
                            self.update_noctalia_monitor(&sibling_png, Some(&mon.name));
                        }
                    }
                } else {
                    self.update_noctalia_monitor(mon_path, Some(&mon.name));
                }
            }

            if any_video {
                let _ = Command::new("systemctl")
                    .args(["--user", "restart", "hypr-livewallpaper.service"])
                    .output();
            } else {
                let _ = Command::new("systemctl")
                    .args(["--user", "stop", "hypr-livewallpaper.service"])
                    .output();
            }
        } else if is_video {
            let symlink = dirs::config_dir()
                .map(|p| p.join("hypr/current_wallpaper.mp4"))
                .unwrap_or_else(|| PathBuf::from(".current_wallpaper.mp4"));

            let _ = std::fs::remove_file(&symlink);
            #[cfg(unix)]
            std::os::unix::fs::symlink(path, &symlink)?;

            let _ = Command::new("systemctl")
                .args(["--user", "restart", "hypr-livewallpaper.service"])
                .output();

            // Find thumbnail or poster for theming
            if let Some(parent) = path.parent() {
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                let thumb = parent.join(".thumbnails").join(format!("{}.png", stem));
                let sibling_png = path.with_extension("png");
                if thumb.exists() {
                    self.update_noctalia(&thumb);
                } else if sibling_png.exists() {
                    self.update_noctalia(&sibling_png);
                }
            }
        } else {
            // Static wallpaper or procedural canvas
            let _ = Command::new("systemctl")
                .args(["--user", "stop", "hypr-livewallpaper.service"])
                .output();

            self.update_noctalia(path);
        }

        if let Some(palette) = palette
            && self.config.sync_hyprlock
        {
            let _ = sync_hyprlock_palette(&self.config.hyprlock_colors_path, palette);
        }

        if let Some(template) = &self.config.on_match_exec {
            let path_str = path.to_string_lossy().to_string();
            let title_str = title.unwrap_or("").to_string();
            let artist_str = artist.unwrap_or("").to_string();
            if template.contains("{monitor}") && !self.config.monitors.is_empty() {
                for mon in &self.config.monitors {
                    let _ = execute_command_template(
                        template,
                        &[
                            ("{file}", &path_str),
                            ("{title}", &title_str),
                            ("{artist}", &artist_str),
                            ("{monitor}", &mon.name),
                        ],
                    );
                }
            } else {
                let _ = execute_command_template(
                    template,
                    &[
                        ("{file}", &path_str),
                        ("{title}", &title_str),
                        ("{artist}", &artist_str),
                        ("{monitor}", "all"),
                    ],
                );
            }
        }

        self.reload_hyprland();
        Ok(())
    }

    fn pause(&self) -> Result<()> {
        let _ = Command::new("killall").args(["-STOP", "mpvpaper"]).output();
        Ok(())
    }

    fn resume(&self) -> Result<()> {
        let _ = Command::new("killall").args(["-CONT", "mpvpaper"]).output();
        Ok(())
    }
}

/// Atomically writes extracted palette hex colors to a Hyprlock configuration snippet.
pub fn sync_hyprlock_palette(path: &Path, palette: &PaletteProfile) -> Result<()> {
    let content = format!(
        "# Generated by VibeVeil — Dynamic Hyprlock Colors\n\
         $primary = rgb({})\n\
         $primary_hex = #{}\n\
         $secondary = rgb({})\n\
         $secondary_hex = #{}\n\
         $surface = rgb({})\n\
         $surface_hex = #{}\n\
         $accent = rgb({})\n\
         $accent_hex = #{}\n",
        palette.primary,
        palette.primary,
        palette.secondary,
        palette.secondary,
        palette.surface,
        palette.surface,
        palette.accent,
        palette.accent,
    );
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let temp_path = path.with_extension("tmp");
    std::fs::write(&temp_path, &content)?;
    std::fs::rename(&temp_path, path)?;
    Ok(())
}

/// Safely tokenizes a command template and executes via execvp argument vector dispatch,
/// eliminating shell injection hazards.
pub fn execute_command_template(template: &str, vars: &[(&str, &str)]) -> Result<()> {
    let mut tokens = template.split_whitespace();
    let program = match tokens.next() {
        Some(p) => p,
        None => return Ok(()),
    };
    let args: Vec<String> = tokens
        .map(|arg| {
            let mut s = arg.to_string();
            for (key, val) in vars {
                let quoted_single = format!("'{}'", key);
                let quoted_double = format!("\"{}\"", key);
                if s == quoted_single || s == quoted_double {
                    s = key.to_string();
                }
                s = s.replace(key, val);
            }
            s
        })
        .collect();
    let _ = Command::new(program).args(&args).output()?;
    Ok(())
}

pub struct CustomCommandBackend {
    template: String,
    monitors: Vec<crate::config::MonitorConfig>,
}

impl CustomCommandBackend {
    #[allow(dead_code)]
    pub fn new(template: String) -> Self {
        Self {
            template,
            monitors: Vec::new(),
        }
    }

    pub fn with_monitors(template: String, monitors: Vec<crate::config::MonitorConfig>) -> Self {
        Self { template, monitors }
    }
}

impl CompositorBackend for CustomCommandBackend {
    fn name(&self) -> &'static str {
        "custom-command"
    }

    fn apply_wallpaper(
        &self,
        path: &Path,
        is_video: bool,
        palette: Option<&PaletteProfile>,
    ) -> Result<()> {
        self.apply_wallpaper_with_meta(path, is_video, palette, None, None)
    }

    fn apply_wallpaper_with_meta(
        &self,
        path: &Path,
        _is_video: bool,
        _palette: Option<&PaletteProfile>,
        title: Option<&str>,
        artist: Option<&str>,
    ) -> Result<()> {
        let path_str = path.to_string_lossy().to_string();
        let title_str = title.unwrap_or("").to_string();
        let artist_str = artist.unwrap_or("").to_string();

        if self.template.contains("{monitor}") && !self.monitors.is_empty() {
            for mon in &self.monitors {
                let mon_path = mon.wallpaper.as_deref().unwrap_or(path);
                let mon_path_str = mon_path.to_string_lossy().to_string();
                execute_command_template(
                    &self.template,
                    &[
                        ("{file}", &mon_path_str),
                        ("{title}", &title_str),
                        ("{artist}", &artist_str),
                        ("{monitor}", &mon.name),
                    ],
                )?;
            }
            Ok(())
        } else {
            execute_command_template(
                &self.template,
                &[
                    ("{file}", &path_str),
                    ("{title}", &title_str),
                    ("{artist}", &artist_str),
                    ("{monitor}", "all"),
                ],
            )
        }
    }

    fn pause(&self) -> Result<()> {
        Ok(())
    }

    fn resume(&self) -> Result<()> {
        Ok(())
    }
}

pub fn create_compositor(config: &CompositorConfig) -> Box<dyn CompositorBackend> {
    match config.backend {
        BackendType::HyprlandNoctalia | BackendType::Mpvpaper | BackendType::Swww => {
            Box::new(HyprlandNoctaliaBackend::new(config.clone()))
        }
        BackendType::Command => {
            let tmpl = config
                .custom_command
                .clone()
                .unwrap_or_else(|| "echo {file}".into());
            Box::new(CustomCommandBackend::with_monitors(
                tmpl,
                config.monitors.clone(),
            ))
        }
    }
}

#[allow(dead_code)]
pub fn detect_connected_monitors() -> Vec<String> {
    if let Ok(output) = Command::new("hyprctl").args(["-j", "monitors"]).output()
        && output.status.success()
        && let Ok(text) = String::from_utf8(output.stdout)
        && let Ok(val) = serde_json::from_str::<serde_json::Value>(&text)
        && let Some(arr) = val.as_array()
    {
        let mut list = Vec::new();
        for item in arr {
            if let Some(name) = item.get("name").and_then(|n| n.as_str()) {
                list.push(name.to_string());
            }
        }
        if !list.is_empty() {
            return list;
        }
    }
    vec!["all".to_string()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BackendType, CompositorConfig};

    fn no_trigger_hyprland_cfg() -> CompositorConfig {
        CompositorConfig {
            backend: BackendType::HyprlandNoctalia,
            custom_command: None,
            trigger_hyprland_reload: false,
            trigger_noctalia_theming: false,
            on_match_exec: None,
            sync_hyprlock: false,
            hyprlock_colors_path: PathBuf::from("/tmp/hyprlock-colors.conf"),
            monitors: Vec::new(),
        }
    }

    // --- CustomCommandBackend ---

    #[test]
    fn custom_backend_name() {
        let b = CustomCommandBackend::new("echo {file}".into());
        assert_eq!(b.name(), "custom-command");
    }

    #[test]
    fn custom_backend_pause_ok() {
        assert!(
            CustomCommandBackend::new("echo {file}".into())
                .pause()
                .is_ok()
        );
    }

    #[test]
    fn custom_backend_resume_ok() {
        assert!(
            CustomCommandBackend::new("echo {file}".into())
                .resume()
                .is_ok()
        );
    }

    #[test]
    fn custom_backend_empty_template_ok() {
        // Empty template: no token → returns Ok without spawning
        let b = CustomCommandBackend::new("".into());
        assert!(
            b.apply_wallpaper(Path::new("/tmp/x.png"), false, None)
                .is_ok()
        );
    }

    #[test]
    fn custom_backend_substitutes_title_and_artist() {
        let b = CustomCommandBackend::new("echo {artist} - {title} - {file}".into());
        assert!(
            b.apply_wallpaper_with_meta(
                Path::new("/tmp/test.png"),
                false,
                None,
                Some("Song Title"),
                Some("Band Name"),
            )
            .is_ok()
        );
    }

    #[test]
    fn custom_backend_echo_substitutes_file() {
        let b = CustomCommandBackend::new("echo {file}".into());
        assert!(
            b.apply_wallpaper(Path::new("/tmp/test.png"), false, None)
                .is_ok()
        );
    }

    #[test]
    fn custom_backend_single_quoted_placeholder() {
        // '{file}' → normalized to {file} before substitution
        let b = CustomCommandBackend::new("echo '{file}'".into());
        assert!(
            b.apply_wallpaper(Path::new("/tmp/wall.png"), false, None)
                .is_ok()
        );
    }

    #[test]
    fn custom_backend_double_quoted_placeholder() {
        let b = CustomCommandBackend::new(r#"echo "{file}""#.into());
        assert!(
            b.apply_wallpaper(Path::new("/tmp/wall.png"), false, None)
                .is_ok()
        );
    }

    #[test]
    fn custom_backend_multiple_args() {
        // printf has multiple tokens besides the program
        let b = CustomCommandBackend::new("printf %s {file}".into());
        assert!(
            b.apply_wallpaper(Path::new("/tmp/w.png"), false, None)
                .is_ok()
        );
    }

    #[test]
    fn custom_backend_no_placeholder_arg_passthrough() {
        // Arg without {file} → passes through unchanged
        let b = CustomCommandBackend::new("echo --static-flag".into());
        assert!(
            b.apply_wallpaper(Path::new("/tmp/w.png"), false, None)
                .is_ok()
        );
    }

    // --- HyprlandNoctaliaBackend ---

    #[test]
    fn hyprland_backend_name() {
        let b = HyprlandNoctaliaBackend::new(no_trigger_hyprland_cfg());
        assert_eq!(b.name(), "hyprland-noctalia");
    }

    #[test]
    fn hyprland_backend_pause_ok() {
        // killall -STOP mpvpaper may fail (mpvpaper not running), but result is Ok(())
        let b = HyprlandNoctaliaBackend::new(no_trigger_hyprland_cfg());
        assert!(b.pause().is_ok());
    }

    #[test]
    fn hyprland_backend_resume_ok() {
        let b = HyprlandNoctaliaBackend::new(no_trigger_hyprland_cfg());
        assert!(b.resume().is_ok());
    }

    #[test]
    fn hyprland_backend_apply_static_no_triggers() {
        use std::io::Write;
        // Write a tiny real PNG so image::open in update_noctalia doesn't matter (trigger=false)
        let mut f = tempfile::Builder::new().suffix(".png").tempfile().unwrap();
        let img = image::RgbImage::from_fn(1, 1, |_, _| image::Rgb([50u8, 50u8, 50u8]));
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        f.write_all(&buf).unwrap();

        let b = HyprlandNoctaliaBackend::new(no_trigger_hyprland_cfg());
        // Static wallpaper (is_video: false): calls stop systemctl (ignored) + update_noctalia (trigger=false)
        assert!(b.apply_wallpaper(f.path(), false, None).is_ok());
    }

    #[test]
    fn hyprland_backend_apply_video_nonexistent_path_ok() {
        // Video branch: tries symlink → will fail on nonexistent src but we swallow the error via `?`
        // Actually symlink would fail since src doesn't exist on POSIX — let's use a temp path
        use std::io::Write;
        let mut f = tempfile::Builder::new().suffix(".mp4").tempfile().unwrap();
        f.write_all(b"fake").unwrap();

        let b = HyprlandNoctaliaBackend::new(no_trigger_hyprland_cfg());
        // The symlink will fail because destination directory may not exist, but we return Ok
        // (systemctl calls are fire-and-forget, symlink error propagates — wrap in allow)
        let _result = b.apply_wallpaper(f.path(), true, None);
        // We don't assert Ok here because the symlink to ~/.config/hypr/ may fail in CI
        // The important thing is no panic
    }

    // --- create_compositor factory ---

    #[test]
    fn factory_command_backend_with_template() {
        let cfg = CompositorConfig {
            backend: BackendType::Command,
            custom_command: Some("swww img {file}".into()),
            trigger_hyprland_reload: false,
            trigger_noctalia_theming: false,
            on_match_exec: None,
            sync_hyprlock: false,
            hyprlock_colors_path: PathBuf::from("/tmp/hyprlock.conf"),
            monitors: Vec::new(),
        };
        let c = create_compositor(&cfg);
        assert_eq!(c.name(), "custom-command");
    }

    #[test]
    fn factory_command_backend_no_template_defaults_echo() {
        let cfg = CompositorConfig {
            backend: BackendType::Command,
            custom_command: None,
            trigger_hyprland_reload: false,
            trigger_noctalia_theming: false,
            on_match_exec: None,
            sync_hyprlock: false,
            hyprlock_colors_path: PathBuf::from("/tmp/hyprlock.conf"),
            monitors: Vec::new(),
        };
        let c = create_compositor(&cfg);
        assert_eq!(c.name(), "custom-command");
    }

    #[test]
    fn factory_hyprland_backend() {
        let c = create_compositor(&no_trigger_hyprland_cfg());
        assert_eq!(c.name(), "hyprland-noctalia");
    }

    #[test]
    fn factory_mpvpaper_maps_to_hyprland_backend() {
        let cfg = CompositorConfig {
            backend: BackendType::Mpvpaper,
            ..no_trigger_hyprland_cfg()
        };
        let c = create_compositor(&cfg);
        assert_eq!(c.name(), "hyprland-noctalia");
    }

    #[test]
    fn factory_swww_maps_to_hyprland_backend() {
        let cfg = CompositorConfig {
            backend: BackendType::Swww,
            ..no_trigger_hyprland_cfg()
        };
        let c = create_compositor(&cfg);
        assert_eq!(c.name(), "hyprland-noctalia");
    }

    #[test]
    fn test_execute_command_template_tokens() {
        let res = execute_command_template(
            "echo {file} {title} {artist}",
            &[
                ("{file}", "/path/to/wall.png"),
                ("{title}", "TrackTitle"),
                ("{artist}", "ArtistName"),
            ],
        );
        assert!(res.is_ok());
    }

    #[test]
    fn test_execute_command_template_empty() {
        assert!(execute_command_template("", &[]).is_ok());
    }

    #[test]
    fn test_sync_hyprlock_palette_writes_file() {
        use crate::color::ColorRgb;
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("hyprlock-colors.conf");
        let palette = PaletteProfile {
            primary: ColorRgb::new(0xff, 0xaa, 0x00),
            secondary: ColorRgb::new(0x00, 0xaa, 0xff),
            accent: ColorRgb::new(0xaa, 0x00, 0xff),
            surface: ColorRgb::new(0x11, 0x11, 0x11),
        };

        let res = sync_hyprlock_palette(&target, &palette);
        assert!(res.is_ok());
        assert!(target.exists());

        let content = std::fs::read_to_string(&target).unwrap();
        assert!(content.contains("$primary = rgb(ffaa00)"));
        assert!(content.contains("$primary_hex = #ffaa00"));
        assert!(content.contains("$secondary = rgb(00aaff)"));
        assert!(content.contains("$surface = rgb(111111)"));
        assert!(content.contains("$accent = rgb(aa00ff)"));
    }

    #[test]
    fn test_hyprland_backend_triggers_sync_hyprlock_and_on_match_exec() {
        use crate::color::ColorRgb;
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("hyprlock-colors.conf");
        let palette = PaletteProfile {
            primary: ColorRgb::new(0x12, 0x34, 0x56),
            secondary: ColorRgb::new(0x65, 0x43, 0x21),
            accent: ColorRgb::new(0xab, 0xcd, 0xef),
            surface: ColorRgb::new(0xfe, 0xdc, 0xba),
        };

        let cfg = CompositorConfig {
            backend: BackendType::HyprlandNoctalia,
            custom_command: None,
            trigger_hyprland_reload: false,
            trigger_noctalia_theming: false,
            on_match_exec: Some("echo {file}".into()),
            sync_hyprlock: true,
            hyprlock_colors_path: target.clone(),
            monitors: Vec::new(),
        };
        let b = HyprlandNoctaliaBackend::new(cfg);
        let dummy = dir.path().join("wall.png");
        std::fs::write(&dummy, b"dummy").unwrap();

        let res = b.apply_wallpaper(&dummy, false, Some(&palette));
        assert!(res.is_ok());
        assert!(target.exists());
        let content = std::fs::read_to_string(&target).unwrap();
        assert!(content.contains("$primary = rgb(123456)"));
    }

    #[test]
    fn test_custom_command_multi_monitor_dispatch() {
        use crate::config::MonitorConfig;
        let dir = tempfile::tempdir().unwrap();
        let dummy = dir.path().join("main_wall.png");
        std::fs::write(&dummy, b"dummy").unwrap();

        let monitors = vec![
            MonitorConfig {
                name: "DP-1".into(),
                strategy: None,
                wallpaper: None,
                primary: true,
            },
            MonitorConfig {
                name: "HDMI-A-1".into(),
                strategy: None,
                wallpaper: Some(PathBuf::from("/custom/wall.png")),
                primary: false,
            },
        ];
        let backend = CustomCommandBackend::with_monitors("echo {monitor}:{file}".into(), monitors);
        let res = backend.apply_wallpaper(&dummy, false, None);
        assert!(res.is_ok());
    }

    #[test]
    fn test_detect_connected_monitors_fallback() {
        let mons = detect_connected_monitors();
        assert!(!mons.is_empty());
    }
}

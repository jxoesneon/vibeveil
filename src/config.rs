use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub strategy: StrategyConfig,
    #[serde(default)]
    pub compositor: CompositorConfig,
    #[serde(default)]
    pub acoustic: AcousticConfig,
    #[serde(default)]
    pub rules: Vec<RuleConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub media_pool: PathBuf,
    #[serde(default = "default_default_wallpaper")]
    pub default_wallpaper: Option<PathBuf>,
    #[serde(default = "default_on_pause")]
    pub on_pause: PauseAction,
    #[serde(default = "default_supported_exts")]
    pub extensions: Vec<String>,
    #[serde(default = "default_max_cache_mb")]
    pub max_cache_mb: u64,
    #[serde(default)]
    pub desktop_notifications: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PauseAction {
    RestoreDefault,
    PausePlayback,
    KeepLast,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyConfig {
    #[serde(default = "default_match_mode")]
    pub mode: MatchMode,
    #[serde(default = "default_color_space")]
    pub color_space: ColorMetric,
    #[serde(default = "default_max_delta_e")]
    pub max_acceptable_delta_e: f32,
    #[serde(default = "default_generate_canvas_fallback")]
    pub procedural_fallback: bool,
    #[serde(default = "default_debounce_ms")]
    pub debounce_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MatchMode {
    Hybrid,
    ColorDistance,
    Rulebook,
    ProceduralCanvas,
    Acoustic,
    Vinyl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ColorMetric {
    Oklab,
    Cielab,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositorConfig {
    #[serde(default = "default_backend")]
    pub backend: BackendType,
    #[serde(default)]
    pub custom_command: Option<String>,
    #[serde(default = "default_true")]
    pub trigger_hyprland_reload: bool,
    #[serde(default = "default_true")]
    pub trigger_noctalia_theming: bool,
    #[serde(default)]
    pub on_match_exec: Option<String>,
    #[serde(default)]
    pub sync_hyprlock: bool,
    #[serde(default = "default_hyprlock_colors_path")]
    pub hyprlock_colors_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcousticConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_acoustic_provider")]
    pub provider: AcousticProvider,
    #[serde(default = "default_acoustic_fallback")]
    pub fallback: AcousticProvider,
    #[serde(default)]
    pub lastfm_api_key: Option<String>,
    #[serde(default = "default_acoustic_cache_dir")]
    pub cache_dir: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AcousticProvider {
    Lastfm,
    Musicbrainz,
    Local,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BackendType {
    HyprlandNoctalia,
    Mpvpaper,
    Swww,
    Command,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleConfig {
    pub match_field: MatchField,
    pub pattern: String,
    pub target_tag_or_path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MatchField {
    Artist,
    Title,
    Album,
    Genre,
    Any,
}

fn default_default_wallpaper() -> Option<PathBuf> {
    dirs::picture_dir().map(|p| p.join("Wallpapers/Live/Pokemon/empoleon.mp4"))
}

fn default_on_pause() -> PauseAction {
    PauseAction::RestoreDefault
}

fn default_supported_exts() -> Vec<String> {
    vec![
        "mp4".into(),
        "mkv".into(),
        "webm".into(),
        "png".into(),
        "jpg".into(),
        "jpeg".into(),
        "webp".into(),
        "gif".into(),
    ]
}

fn default_match_mode() -> MatchMode {
    MatchMode::Hybrid
}

fn default_color_space() -> ColorMetric {
    ColorMetric::Oklab
}

fn default_max_delta_e() -> f32 {
    45.0
}

fn default_generate_canvas_fallback() -> bool {
    true
}

fn default_backend() -> BackendType {
    BackendType::HyprlandNoctalia
}

fn default_true() -> bool {
    true
}

fn default_max_cache_mb() -> u64 {
    200
}

fn default_debounce_ms() -> u64 {
    250
}

fn default_hyprlock_colors_path() -> PathBuf {
    dirs::config_dir()
        .map(|p| p.join("hypr/hyprlock-colors.conf"))
        .unwrap_or_else(|| PathBuf::from("hyprlock-colors.conf"))
}

fn default_acoustic_provider() -> AcousticProvider {
    AcousticProvider::Lastfm
}

fn default_acoustic_fallback() -> AcousticProvider {
    AcousticProvider::Musicbrainz
}

fn default_acoustic_cache_dir() -> PathBuf {
    dirs::cache_dir()
        .map(|p| p.join("vibeveil/acoustic"))
        .unwrap_or_else(|| PathBuf::from(".cache/vibeveil/acoustic"))
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            media_pool: dirs::picture_dir()
                .map(|p| p.join("Wallpapers"))
                .unwrap_or_else(|| PathBuf::from("./wallpapers")),
            default_wallpaper: default_default_wallpaper(),
            on_pause: default_on_pause(),
            extensions: default_supported_exts(),
            max_cache_mb: default_max_cache_mb(),
            desktop_notifications: false,
        }
    }
}

impl Default for StrategyConfig {
    fn default() -> Self {
        Self {
            mode: default_match_mode(),
            color_space: default_color_space(),
            max_acceptable_delta_e: default_max_delta_e(),
            procedural_fallback: default_generate_canvas_fallback(),
            debounce_ms: default_debounce_ms(),
        }
    }
}

impl Default for CompositorConfig {
    fn default() -> Self {
        Self {
            backend: default_backend(),
            custom_command: None,
            trigger_hyprland_reload: true,
            trigger_noctalia_theming: true,
            on_match_exec: None,
            sync_hyprlock: false,
            hyprlock_colors_path: default_hyprlock_colors_path(),
        }
    }
}

impl Default for AcousticConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: default_acoustic_provider(),
            fallback: default_acoustic_fallback(),
            lastfm_api_key: None,
            cache_dir: default_acoustic_cache_dir(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            strategy: StrategyConfig::default(),
            compositor: CompositorConfig::default(),
            acoustic: AcousticConfig::default(),
            rules: vec![
                RuleConfig {
                    match_field: MatchField::Any,
                    pattern: "(?i)(lo-?fi|chill|ambient|nature)".into(),
                    target_tag_or_path: "ghibli_nature".into(),
                },
                RuleConfig {
                    match_field: MatchField::Any,
                    pattern: "(?i)(metal|rock|battle|doom|epic)".into(),
                    target_tag_or_path: "groudon".into(),
                },
                RuleConfig {
                    match_field: MatchField::Any,
                    pattern: "(?i)(synthwave|cyber|electro|night)".into(),
                    target_tag_or_path: "cyberpunk_lucy".into(),
                },
                RuleConfig {
                    match_field: MatchField::Any,
                    pattern: "(?i)(anime|j-?pop|fast|electric)".into(),
                    target_tag_or_path: "demon_slayer_zenitsu".into(),
                },
            ],
        }
    }
}

impl Config {
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .map(|p| p.join("vibeveil/config.toml"))
            .unwrap_or_else(|| PathBuf::from("config.toml"))
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists()
            && let Ok(content) = std::fs::read_to_string(&path)
            && let Ok(parsed) = toml::from_str::<Config>(&content)
        {
            return parsed;
        }
        Self::default()
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let serialized = toml::to_string_pretty(self)?;
        std::fs::write(&path, serialized)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- GeneralConfig defaults ---

    #[test]
    fn general_config_default_max_cache_mb() {
        assert_eq!(GeneralConfig::default().max_cache_mb, 200);
    }

    #[test]
    fn general_config_default_extensions_nonempty() {
        let exts = GeneralConfig::default().extensions;
        assert!(!exts.is_empty());
        assert!(exts.contains(&"mp4".to_string()));
        assert!(exts.contains(&"png".to_string()));
        assert!(exts.contains(&"jpg".to_string()));
        assert!(exts.contains(&"mkv".to_string()));
        assert!(exts.contains(&"webm".to_string()));
        assert!(exts.contains(&"jpeg".to_string()));
        assert!(exts.contains(&"webp".to_string()));
        assert!(exts.contains(&"gif".to_string()));
    }

    #[test]
    fn general_config_default_on_pause_is_restore() {
        assert_eq!(
            GeneralConfig::default().on_pause,
            PauseAction::RestoreDefault
        );
    }

    // --- StrategyConfig defaults ---

    #[test]
    fn strategy_config_default_debounce_ms() {
        assert_eq!(StrategyConfig::default().debounce_ms, 250);
    }

    #[test]
    fn strategy_config_default_max_delta_e() {
        assert_eq!(StrategyConfig::default().max_acceptable_delta_e, 45.0);
    }

    #[test]
    fn strategy_config_default_procedural_fallback_true() {
        assert!(StrategyConfig::default().procedural_fallback);
    }

    #[test]
    fn strategy_config_default_mode_hybrid() {
        assert_eq!(StrategyConfig::default().mode, MatchMode::Hybrid);
    }

    #[test]
    fn strategy_config_default_color_space_oklab() {
        assert_eq!(StrategyConfig::default().color_space, ColorMetric::Oklab);
    }

    // --- CompositorConfig defaults ---

    #[test]
    fn compositor_config_default_backend_hyprland() {
        assert_eq!(
            CompositorConfig::default().backend,
            BackendType::HyprlandNoctalia
        );
    }

    #[test]
    fn compositor_config_default_triggers_true() {
        let c = CompositorConfig::default();
        assert!(c.trigger_hyprland_reload);
        assert!(c.trigger_noctalia_theming);
    }

    #[test]
    fn compositor_config_default_no_custom_command() {
        assert!(CompositorConfig::default().custom_command.is_none());
    }

    // --- Config::default ---

    #[test]
    fn config_default_has_four_rules() {
        assert_eq!(Config::default().rules.len(), 4);
    }

    #[test]
    fn config_default_rules_have_patterns() {
        for rule in Config::default().rules {
            assert!(!rule.pattern.is_empty());
            assert!(!rule.target_tag_or_path.is_empty());
        }
    }

    // --- Enum serde round-trips via serde_json (avoids TOML's bare-enum limitation) ---
    // TOML cannot serialize bare enum values (only inside a table context).
    // We use serde_json for pure enum codec tests; TOML table-embedded tests follow below.

    #[test]
    fn pause_action_roundtrip_all_variants() {
        for v in [
            PauseAction::RestoreDefault,
            PauseAction::PausePlayback,
            PauseAction::KeepLast,
        ] {
            let s = serde_json::to_string(&v).unwrap();
            let back: PauseAction = serde_json::from_str(&s).unwrap();
            assert_eq!(back, v);
        }
    }

    #[test]
    fn match_mode_roundtrip_all_variants() {
        for v in [
            MatchMode::Hybrid,
            MatchMode::Rulebook,
            MatchMode::ColorDistance,
            MatchMode::ProceduralCanvas,
            MatchMode::Acoustic,
            MatchMode::Vinyl,
        ] {
            let s = serde_json::to_string(&v).unwrap();
            let back: MatchMode = serde_json::from_str(&s).unwrap();
            assert_eq!(back, v);
        }
    }

    #[test]
    fn color_metric_roundtrip_all_variants() {
        for v in [ColorMetric::Oklab, ColorMetric::Cielab] {
            let s = serde_json::to_string(&v).unwrap();
            let back: ColorMetric = serde_json::from_str(&s).unwrap();
            assert_eq!(back, v);
        }
    }

    #[test]
    fn backend_type_roundtrip_all_variants() {
        for v in [
            BackendType::HyprlandNoctalia,
            BackendType::Mpvpaper,
            BackendType::Swww,
            BackendType::Command,
        ] {
            let s = serde_json::to_string(&v).unwrap();
            let back: BackendType = serde_json::from_str(&s).unwrap();
            assert_eq!(back, v);
        }
    }

    #[test]
    fn match_field_roundtrip_all_variants() {
        for v in [
            MatchField::Artist,
            MatchField::Title,
            MatchField::Album,
            MatchField::Genre,
            MatchField::Any,
        ] {
            let s = serde_json::to_string(&v).unwrap();
            let back: MatchField = serde_json::from_str(&s).unwrap();
            assert_eq!(back, v);
        }
    }

    // Verify TOML uses kebab-case strings for embedded enum values
    #[test]
    fn toml_enum_values_in_table_context() {
        let toml_str = "[strategy]\nmode = \"color-distance\"\ncolor_space = \"cielab\"\n";
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.strategy.mode, MatchMode::ColorDistance);
        assert_eq!(cfg.strategy.color_space, ColorMetric::Cielab);
    }

    #[test]
    fn toml_pause_action_in_table_context() {
        let toml_str = "[general]\nmedia_pool = \"/tmp\"\non_pause = \"keep-last\"\n";
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.general.on_pause, PauseAction::KeepLast);
    }

    // --- TOML partial override uses defaults for missing keys ---

    #[test]
    fn toml_partial_strategy_override() {
        let toml_str = "[strategy]\ndebounce_ms = 500\nmax_acceptable_delta_e = 30.0\n";
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.strategy.debounce_ms, 500);
        assert_eq!(cfg.strategy.max_acceptable_delta_e, 30.0);
        assert_eq!(cfg.strategy.mode, MatchMode::Hybrid); // default preserved
    }

    #[test]
    fn toml_partial_general_override() {
        let toml_str = "[general]\nmax_cache_mb = 512\nmedia_pool = \"/tmp\"\n";
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.general.max_cache_mb, 512);
    }

    #[test]
    fn toml_compositor_command_backend() {
        let toml_str =
            "[compositor]\nbackend = \"command\"\ncustom_command = \"swww img {file}\"\n";
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.compositor.backend, BackendType::Command);
        assert_eq!(
            cfg.compositor.custom_command.as_deref(),
            Some("swww img {file}")
        );
    }

    // --- Config full serialization round-trip ---

    #[test]
    fn config_full_toml_roundtrip() {
        let original = Config::default();
        let serialized = toml::to_string_pretty(&original).unwrap();
        let parsed: Config = toml::from_str(&serialized).unwrap();
        assert_eq!(parsed.general.max_cache_mb, original.general.max_cache_mb);
        assert_eq!(parsed.strategy.debounce_ms, original.strategy.debounce_ms);
        assert_eq!(
            parsed.strategy.max_acceptable_delta_e,
            original.strategy.max_acceptable_delta_e
        );
        assert_eq!(parsed.rules.len(), original.rules.len());
        assert_eq!(parsed.compositor.backend, original.compositor.backend);
    }

    // --- Config::config_path ---

    #[test]
    fn config_path_is_nonempty() {
        let p = Config::config_path();
        assert!(!p.as_os_str().is_empty());
        assert!(p.to_string_lossy().contains("vibeveil"));
    }

    #[test]
    fn general_config_default_notifications_false() {
        assert!(!GeneralConfig::default().desktop_notifications);
    }

    #[test]
    fn compositor_config_default_sync_hyprlock_false() {
        let c = CompositorConfig::default();
        assert!(!c.sync_hyprlock);
        assert!(c.on_match_exec.is_none());
        assert!(!c.hyprlock_colors_path.as_os_str().is_empty());
    }

    #[test]
    fn acoustic_config_defaults() {
        let a = AcousticConfig::default();
        assert!(!a.enabled);
        assert_eq!(a.provider, AcousticProvider::Lastfm);
        assert_eq!(a.fallback, AcousticProvider::Musicbrainz);
        assert!(a.lastfm_api_key.is_none());
        assert!(!a.cache_dir.as_os_str().is_empty());
    }

    #[test]
    fn acoustic_provider_roundtrip_all_variants() {
        for v in [
            AcousticProvider::Lastfm,
            AcousticProvider::Musicbrainz,
            AcousticProvider::Local,
        ] {
            let s = serde_json::to_string(&v).unwrap();
            let back: AcousticProvider = serde_json::from_str(&s).unwrap();
            assert_eq!(back, v);
        }
    }

    #[test]
    fn toml_new_features_override() {
        let toml_str = r#"
[general]
media_pool = "/tmp"
desktop_notifications = true

[compositor]
on_match_exec = "notify-send '{title}' '{artist}'"
sync_hyprlock = true
hyprlock_colors_path = "/tmp/hyprlock-colors.conf"

[acoustic]
enabled = true
provider = "musicbrainz"
fallback = "local"
"#;
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert!(cfg.general.desktop_notifications);
        assert_eq!(
            cfg.compositor.on_match_exec.as_deref(),
            Some("notify-send '{title}' '{artist}'")
        );
        assert!(cfg.compositor.sync_hyprlock);
        assert_eq!(
            cfg.compositor.hyprlock_colors_path,
            PathBuf::from("/tmp/hyprlock-colors.conf")
        );
        assert!(cfg.acoustic.enabled);
        assert_eq!(cfg.acoustic.provider, AcousticProvider::Musicbrainz);
        assert_eq!(cfg.acoustic.fallback, AcousticProvider::Local);
    }
}

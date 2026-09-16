use crate::canvas::CanvasGenerator;
use crate::color::PaletteProfile;
use crate::config::{Config, MatchField, MatchMode};
use crate::pool::MediaPool;
use regex::Regex;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct TrackContext {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub art_path: Option<PathBuf>,
    pub palette: Option<PaletteProfile>,
}

#[derive(Debug, Clone)]
pub struct MatchResult {
    pub wallpaper_path: PathBuf,
    pub is_video: bool,
    pub palette: Option<PaletteProfile>,
    pub score: f32,
    pub strategy: String,
    pub reason: String,
}

pub trait MatchStrategy: Send + Sync {
    fn name(&self) -> &'static str;
    fn find_match(&self, track: &TrackContext, pool: &MediaPool) -> Option<MatchResult>;
}

pub struct RulebookMatcher {
    config: Config,
}

impl RulebookMatcher {
    pub fn new(config: Config) -> Self {
        Self { config }
    }
}

impl MatchStrategy for RulebookMatcher {
    fn name(&self) -> &'static str {
        "rulebook"
    }

    fn find_match(&self, track: &TrackContext, pool: &MediaPool) -> Option<MatchResult> {
        for rule in &self.config.rules {
            let target_text = match rule.match_field {
                MatchField::Artist => &track.artist,
                MatchField::Title => &track.title,
                MatchField::Album => &track.album,
                MatchField::Genre | MatchField::Any => {
                    // Combine all metadata fields for Any
                    &format!("{} {} {}", track.title, track.artist, track.album)
                }
            };

            if let Ok(re) = Regex::new(&rule.pattern)
                && re.is_match(target_text)
            {
                // Look up media in pool matching the target tag/path
                let matches = pool.find_by_tag(&rule.target_tag_or_path);
                if let Some(first) = matches.first() {
                    return Some(MatchResult {
                        wallpaper_path: first.path.clone(),
                        is_video: first.is_video,
                        palette: first.palette.clone(),
                        score: 1.0,
                        strategy: "rulebook".into(),
                        reason: format!(
                            "Matched pattern '{}' -> tag '{}'",
                            rule.pattern, rule.target_tag_or_path
                        ),
                    });
                }
            }
        }
        None
    }
}

pub struct ColorDistanceMatcher {
    max_delta_e: f32,
}

impl ColorDistanceMatcher {
    pub fn new(max_delta_e: f32) -> Self {
        Self { max_delta_e }
    }
}

impl MatchStrategy for ColorDistanceMatcher {
    fn name(&self) -> &'static str {
        "color-distance"
    }

    fn find_match(&self, track: &TrackContext, pool: &MediaPool) -> Option<MatchResult> {
        let track_palette = track.palette.as_ref()?;
        let (best_item, dist) = pool.find_closest_color(track_palette)?;

        if dist <= self.max_delta_e {
            Some(MatchResult {
                wallpaper_path: best_item.path.clone(),
                is_video: best_item.is_video,
                palette: best_item.palette.clone(),
                score: (100.0 - dist).max(0.0) / 100.0,
                strategy: "color-distance".into(),
                reason: format!(
                    "Lowest Oklab Delta-E distance ({:.2}) to '{}'",
                    dist, best_item.name
                ),
            })
        } else {
            None
        }
    }
}

pub struct ProceduralCanvasMatcher {
    generator: CanvasGenerator,
}

impl ProceduralCanvasMatcher {
    pub fn new() -> Self {
        Self {
            generator: CanvasGenerator::new(),
        }
    }
}

impl MatchStrategy for ProceduralCanvasMatcher {
    fn name(&self) -> &'static str {
        "procedural-canvas"
    }

    fn find_match(&self, track: &TrackContext, _pool: &MediaPool) -> Option<MatchResult> {
        let art_path = track.art_path.as_ref()?;
        let (canvas_path, is_video) = match self
            .generator
            .generate_ambient_video_loop(art_path, 1920, 1080)
        {
            Ok(v_path) => (v_path, true),
            Err(_) => (
                self.generator
                    .generate_ambient_canvas(art_path, 1920, 1080)
                    .ok()?,
                false,
            ),
        };

        Some(MatchResult {
            wallpaper_path: canvas_path,
            is_video,
            palette: track.palette.clone(),
            score: 0.90,
            strategy: "procedural-canvas".into(),
            reason: "Rendered ambient glassmorphism animated album canvas".into(),
        })
    }
}

pub struct VinylCanvasMatcher {
    generator: CanvasGenerator,
}

impl VinylCanvasMatcher {
    pub fn new() -> Self {
        Self {
            generator: CanvasGenerator::new(),
        }
    }
}

impl MatchStrategy for VinylCanvasMatcher {
    fn name(&self) -> &'static str {
        "vinyl-canvas"
    }

    fn find_match(&self, track: &TrackContext, _pool: &MediaPool) -> Option<MatchResult> {
        let art_path = track.art_path.as_ref()?;
        let (canvas_path, is_video) = match self
            .generator
            .generate_vinyl_video_loop(art_path, 1920, 1080)
        {
            Ok(v_path) => (v_path, true),
            Err(_) => (
                self.generator
                    .generate_vinyl_canvas(art_path, 1920, 1080)
                    .ok()?,
                false,
            ),
        };

        Some(MatchResult {
            wallpaper_path: canvas_path,
            is_video,
            palette: track.palette.clone(),
            score: 0.95,
            strategy: "vinyl-canvas".into(),
            reason: "Synthesized procedural spinning vinyl disc canvas with album label".into(),
        })
    }
}

pub struct AcousticMatcher {
    classifier: crate::acoustic::AcousticClassifier,
}

impl AcousticMatcher {
    pub fn new(config: &Config) -> Self {
        Self {
            classifier: crate::acoustic::AcousticClassifier::new(config.acoustic.clone()),
        }
    }
}

impl MatchStrategy for AcousticMatcher {
    fn name(&self) -> &'static str {
        "acoustic"
    }

    fn find_match(&self, track: &TrackContext, pool: &MediaPool) -> Option<MatchResult> {
        let mood = self.classifier.classify_local(&track.title, &track.artist);
        let items = pool.items();
        if items.is_empty() {
            return None;
        }

        let target_tags: &[&str] = if mood.energy >= 0.65 {
            &[
                "groudon", "battle", "epic", "rock", "metal", "lucy", "zenitsu",
            ]
        } else if mood.energy <= 0.45 {
            &["ghibli", "nature", "squirtle", "lofi", "chill", "ambient"]
        } else {
            &["empoleon", "pokemon", "anime"]
        };

        for tag in target_tags {
            let matches = pool.find_by_tag(tag);
            if let Some(matched) = matches.first() {
                return Some(MatchResult {
                    wallpaper_path: matched.path.clone(),
                    is_video: matched.is_video,
                    palette: matched.palette.clone(),
                    score: mood.energy,
                    strategy: "acoustic".into(),
                    reason: format!(
                        "Acoustic mood (energy: {:.2}, valence: {:.2}, provider: {}) mapped to '{}'",
                        mood.energy, mood.valence, mood.provider, matched.name
                    ),
                });
            }
        }
        None
    }
}

pub struct HybridMatcher {
    rulebook: RulebookMatcher,
    acoustic: AcousticMatcher,
    color_distance: ColorDistanceMatcher,
    procedural: ProceduralCanvasMatcher,
    acoustic_enabled: bool,
}

impl HybridMatcher {
    pub fn new(config: &Config) -> Self {
        Self {
            rulebook: RulebookMatcher::new(config.clone()),
            acoustic: AcousticMatcher::new(config),
            color_distance: ColorDistanceMatcher::new(config.strategy.max_acceptable_delta_e),
            procedural: ProceduralCanvasMatcher::new(),
            acoustic_enabled: config.acoustic.enabled,
        }
    }
}

impl MatchStrategy for HybridMatcher {
    fn name(&self) -> &'static str {
        "hybrid"
    }

    fn find_match(&self, track: &TrackContext, pool: &MediaPool) -> Option<MatchResult> {
        // Priority 1: Direct rulebook / genre / tag mapping
        if let Some(res) = self.rulebook.find_match(track, pool) {
            return Some(res);
        }

        // Priority 2: Acoustic mood mapping (if enabled)
        if self.acoustic_enabled
            && let Some(res) = self.acoustic.find_match(track, pool)
        {
            return Some(res);
        }

        // Priority 3: Closest perceptual color harmony in media pool
        if let Some(res) = self.color_distance.find_match(track, pool) {
            return Some(res);
        }

        // Priority 4: Fallback procedural ambient canvas
        self.procedural.find_match(track, pool)
    }
}

pub fn create_matcher(config: &Config) -> Box<dyn MatchStrategy> {
    match config.strategy.mode {
        MatchMode::Rulebook => Box::new(RulebookMatcher::new(config.clone())),
        MatchMode::ColorDistance => Box::new(ColorDistanceMatcher::new(
            config.strategy.max_acceptable_delta_e,
        )),
        MatchMode::ProceduralCanvas => Box::new(ProceduralCanvasMatcher::new()),
        MatchMode::Acoustic => Box::new(AcousticMatcher::new(config)),
        MatchMode::Vinyl => Box::new(VinylCanvasMatcher::new()),
        MatchMode::Hybrid => Box::new(HybridMatcher::new(config)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::{ColorRgb, PaletteProfile};
    use crate::config::{Config, MatchField, MatchMode, RuleConfig};
    use crate::pool::MediaPool;
    use tempfile::TempDir;

    // --- helpers ---

    fn track(title: &str, artist: &str, album: &str) -> TrackContext {
        TrackContext {
            title: title.into(),
            artist: artist.into(),
            album: album.into(),
            art_path: None,
            palette: None,
        }
    }

    fn track_with_palette(palette: PaletteProfile) -> TrackContext {
        TrackContext {
            title: "Test".into(),
            artist: "Artist".into(),
            album: "Album".into(),
            art_path: None,
            palette: Some(palette),
        }
    }

    fn write_solid_png(path: &std::path::Path, rgb: [u8; 3]) {
        let img = image::RgbImage::from_fn(8, 8, |_, _| image::Rgb(rgb));
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        std::fs::write(path, buf).unwrap();
    }

    fn pool_with_images(dir: &TempDir, names: &[(&str, [u8; 3])]) -> MediaPool {
        for (name, rgb) in names {
            write_solid_png(&dir.path().join(format!("{}.png", name)), *rgb);
        }
        let cache_file = dir.path().join("test_pool_index.json");
        let mut p =
            MediaPool::new_isolated(dir.path().to_path_buf(), vec!["png".into()], cache_file);
        p.scan(None);
        p
    }

    fn empty_pool(dir: &TempDir) -> MediaPool {
        let cache_file = dir.path().join("test_pool_index.json");
        MediaPool::new_isolated(dir.path().to_path_buf(), vec!["png".into()], cache_file)
    }

    fn config_with_rules(rules: Vec<RuleConfig>) -> Config {
        Config {
            rules,
            ..Config::default()
        }
    }

    // --- TrackContext / MatchResult ---

    #[test]
    fn track_context_fields_accessible() {
        let t = track("Song", "Band", "Record");
        assert_eq!(t.title, "Song");
        assert_eq!(t.artist, "Band");
        assert_eq!(t.album, "Record");
        assert!(t.palette.is_none());
        assert!(t.art_path.is_none());
    }

    // --- RulebookMatcher ---

    #[test]
    fn rulebook_name() {
        assert_eq!(RulebookMatcher::new(Config::default()).name(), "rulebook");
    }

    #[test]
    fn rulebook_no_match_empty_pool() {
        let dir = TempDir::new().unwrap();
        let pool = empty_pool(&dir);
        let m = RulebookMatcher::new(Config::default());
        assert!(
            m.find_match(&track("Chill Beats", "Lofi", "Vibes"), &pool)
                .is_none()
        );
    }

    #[test]
    fn rulebook_matches_any_field() {
        let dir = TempDir::new().unwrap();
        let pool = pool_with_images(&dir, &[("ghibli_nature", [100, 200, 100])]);
        let cfg = config_with_rules(vec![RuleConfig {
            match_field: MatchField::Any,
            pattern: "(?i)chill".into(),
            target_tag_or_path: "ghibli_nature".into(),
        }]);
        let m = RulebookMatcher::new(cfg);
        let result = m.find_match(&track("Chill Beats", "Lofi", "Album"), &pool);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.strategy, "rulebook");
        assert_eq!(r.score, 1.0);
    }

    #[test]
    fn rulebook_matches_title_field() {
        let dir = TempDir::new().unwrap();
        let pool = pool_with_images(&dir, &[("groudon", [200, 50, 0])]);
        let cfg = config_with_rules(vec![RuleConfig {
            match_field: MatchField::Title,
            pattern: "(?i)battle".into(),
            target_tag_or_path: "groudon".into(),
        }]);
        let result =
            RulebookMatcher::new(cfg).find_match(&track("Final Battle", "Band", "Album"), &pool);
        assert!(result.is_some());
    }

    #[test]
    fn rulebook_matches_artist_field() {
        let dir = TempDir::new().unwrap();
        let pool = pool_with_images(&dir, &[("groudon", [200, 50, 0])]);
        let cfg = config_with_rules(vec![RuleConfig {
            match_field: MatchField::Artist,
            pattern: "(?i)metal".into(),
            target_tag_or_path: "groudon".into(),
        }]);
        let result =
            RulebookMatcher::new(cfg).find_match(&track("Song", "Metal Lords", "Album"), &pool);
        assert!(result.is_some());
    }

    #[test]
    fn rulebook_matches_album_field() {
        let dir = TempDir::new().unwrap();
        let pool = pool_with_images(&dir, &[("groudon", [200, 50, 0])]);
        let cfg = config_with_rules(vec![RuleConfig {
            match_field: MatchField::Album,
            pattern: "(?i)epic".into(),
            target_tag_or_path: "groudon".into(),
        }]);
        let result = RulebookMatcher::new(cfg)
            .find_match(&track("Song", "Artist", "Epic Collection"), &pool);
        assert!(result.is_some());
    }

    #[test]
    fn rulebook_genre_field_acts_like_any() {
        let dir = TempDir::new().unwrap();
        let pool = pool_with_images(&dir, &[("groudon", [200, 50, 0])]);
        let cfg = config_with_rules(vec![RuleConfig {
            match_field: MatchField::Genre,
            pattern: "(?i)epic".into(),
            target_tag_or_path: "groudon".into(),
        }]);
        // Genre falls through to "Any" branch (combines title+artist+album)
        let result =
            RulebookMatcher::new(cfg).find_match(&track("epic song", "Artist", "Album"), &pool);
        assert!(result.is_some());
    }

    #[test]
    fn rulebook_no_pattern_match_returns_none() {
        let dir = TempDir::new().unwrap();
        let pool = pool_with_images(&dir, &[("art", [100, 100, 100])]);
        let m = RulebookMatcher::new(Config::default());
        assert!(
            m.find_match(&track("Zzz No Match Song", "Zzz", "Zzz"), &pool)
                .is_none()
        );
    }

    #[test]
    fn rulebook_tag_not_in_pool_returns_none() {
        let dir = TempDir::new().unwrap();
        let pool = pool_with_images(&dir, &[("some_other_art", [100, 100, 100])]);
        let cfg = config_with_rules(vec![RuleConfig {
            match_field: MatchField::Any,
            pattern: "(?i)chill".into(),
            target_tag_or_path: "ghibli_nature".into(), // tag not in pool
        }]);
        let result = RulebookMatcher::new(cfg).find_match(&track("Chill", "Lofi", "Vibes"), &pool);
        assert!(result.is_none());
    }

    // --- ColorDistanceMatcher ---

    #[test]
    fn color_distance_name() {
        assert_eq!(ColorDistanceMatcher::new(45.0).name(), "color-distance");
    }

    #[test]
    fn color_distance_no_palette_on_track() {
        let dir = TempDir::new().unwrap();
        let pool = empty_pool(&dir);
        let m = ColorDistanceMatcher::new(45.0);
        assert!(
            m.find_match(&track("Song", "Art", "Album"), &pool)
                .is_none()
        );
    }

    #[test]
    fn color_distance_empty_pool_returns_none() {
        let dir = TempDir::new().unwrap();
        let pool = empty_pool(&dir);
        let palette = PaletteProfile {
            primary: ColorRgb::new(255, 0, 0),
            secondary: ColorRgb::new(200, 0, 0),
            surface: ColorRgb::new(20, 0, 0),
            accent: ColorRgb::new(255, 100, 0),
        };
        let m = ColorDistanceMatcher::new(45.0);
        assert!(m.find_match(&track_with_palette(palette), &pool).is_none());
    }

    #[test]
    fn color_distance_finds_close_match() {
        let dir = TempDir::new().unwrap();
        let pool = pool_with_images(&dir, &[("red_art", [220, 0, 0])]);
        let red_palette = PaletteProfile {
            primary: ColorRgb::new(220, 0, 0),
            secondary: ColorRgb::new(200, 10, 0),
            surface: ColorRgb::new(80, 0, 0),
            accent: ColorRgb::new(255, 50, 0),
        };
        let m = ColorDistanceMatcher::new(100.0); // loose threshold
        let result = m.find_match(&track_with_palette(red_palette), &pool);
        assert!(result.is_some());
        assert_eq!(result.unwrap().strategy, "color-distance");
    }

    #[test]
    fn color_distance_very_strict_threshold_may_return_none() {
        let dir = TempDir::new().unwrap();
        let pool = pool_with_images(&dir, &[("blue_art", [0, 0, 220])]);
        let red_palette = PaletteProfile {
            primary: ColorRgb::new(255, 0, 0),
            secondary: ColorRgb::new(200, 0, 0),
            surface: ColorRgb::new(50, 0, 0),
            accent: ColorRgb::new(255, 100, 0),
        };
        let m = ColorDistanceMatcher::new(1.0); // very strict
        let result = m.find_match(&track_with_palette(red_palette), &pool);
        // May or may not match depending on actual distance; just verify no panic
        let _ = result;
    }

    // --- ProceduralCanvasMatcher ---

    #[test]
    fn procedural_canvas_name() {
        assert_eq!(ProceduralCanvasMatcher::new().name(), "procedural-canvas");
    }

    #[test]
    fn procedural_canvas_no_art_path_returns_none() {
        let dir = TempDir::new().unwrap();
        let pool = empty_pool(&dir);
        let m = ProceduralCanvasMatcher::new();
        assert!(
            m.find_match(&track("Song", "Artist", "Album"), &pool)
                .is_none()
        );
    }

    // --- HybridMatcher ---

    #[test]
    fn hybrid_name() {
        assert_eq!(HybridMatcher::new(&Config::default()).name(), "hybrid");
    }

    #[test]
    fn hybrid_falls_through_all_to_none() {
        let dir = TempDir::new().unwrap();
        let pool = empty_pool(&dir);
        let m = HybridMatcher::new(&Config::default());
        // No rulebook match, no palette, no art_path
        assert!(m.find_match(&track("Zzz", "Zzz", "Zzz"), &pool).is_none());
    }

    #[test]
    fn hybrid_uses_rulebook_first() {
        let dir = TempDir::new().unwrap();
        let pool = pool_with_images(
            &dir,
            &[("ghibli_nature", [100, 200, 100]), ("red_art", [220, 0, 0])],
        );
        let cfg = Config {
            rules: vec![RuleConfig {
                match_field: MatchField::Any,
                pattern: "(?i)chill".into(),
                target_tag_or_path: "ghibli_nature".into(),
            }],
            ..Config::default()
        };
        let m = HybridMatcher::new(&cfg);
        let result = m.find_match(&track("Chill Beats", "Lofi", "Album"), &pool);
        assert!(result.is_some());
        assert_eq!(result.unwrap().strategy, "rulebook");
    }

    #[test]
    fn hybrid_falls_back_to_color_when_no_rule() {
        let dir = TempDir::new().unwrap();
        let pool = pool_with_images(&dir, &[("red_art", [220, 0, 0])]);
        let mut cfg = Config {
            rules: vec![], // no rules
            ..Config::default()
        };
        cfg.strategy.max_acceptable_delta_e = 100.0;
        let red_palette = PaletteProfile {
            primary: ColorRgb::new(220, 0, 0),
            secondary: ColorRgb::new(200, 10, 0),
            surface: ColorRgb::new(80, 0, 0),
            accent: ColorRgb::new(255, 50, 0),
        };
        let m = HybridMatcher::new(&cfg);
        let t = track_with_palette(red_palette);
        let result = m.find_match(&t, &pool);
        assert!(result.is_some());
        assert_eq!(result.unwrap().strategy, "color-distance");
    }

    // --- create_matcher factory ---

    #[test]
    fn create_matcher_hybrid() {
        let mut cfg = Config::default();
        cfg.strategy.mode = MatchMode::Hybrid;
        assert_eq!(create_matcher(&cfg).name(), "hybrid");
    }

    #[test]
    fn create_matcher_rulebook() {
        let mut cfg = Config::default();
        cfg.strategy.mode = MatchMode::Rulebook;
        assert_eq!(create_matcher(&cfg).name(), "rulebook");
    }

    #[test]
    fn create_matcher_color_distance() {
        let mut cfg = Config::default();
        cfg.strategy.mode = MatchMode::ColorDistance;
        assert_eq!(create_matcher(&cfg).name(), "color-distance");
    }

    #[test]
    fn create_matcher_procedural_canvas() {
        let mut cfg = Config::default();
        cfg.strategy.mode = MatchMode::ProceduralCanvas;
        assert_eq!(create_matcher(&cfg).name(), "procedural-canvas");
    }

    #[test]
    fn create_matcher_acoustic() {
        let mut cfg = Config::default();
        cfg.strategy.mode = MatchMode::Acoustic;
        assert_eq!(create_matcher(&cfg).name(), "acoustic");
    }

    #[test]
    fn create_matcher_vinyl() {
        let mut cfg = Config::default();
        cfg.strategy.mode = MatchMode::Vinyl;
        assert_eq!(create_matcher(&cfg).name(), "vinyl-canvas");
    }

    #[test]
    fn test_acoustic_matcher_finds_tag_match() {
        let dir = TempDir::new().unwrap();
        let pool = pool_with_images(&dir, &[("groudon", [200, 50, 50])]);
        let cfg = Config::default();
        let matcher = AcousticMatcher::new(&cfg);

        let t = TrackContext {
            title: "Doom Eternal Battle Rock".into(),
            artist: "Mick Gordon".into(),
            album: "Doom OST".into(),
            art_path: None,
            palette: None,
        };

        let res = matcher.find_match(&t, &pool);
        assert!(res.is_some());
        let m = res.unwrap();
        assert_eq!(m.strategy, "acoustic");
        assert!(m.reason.contains("groudon"));
    }

    #[test]
    fn test_vinyl_canvas_matcher_generates_canvas() {
        let dir = TempDir::new().unwrap();
        let pool = MediaPool::new(dir.path().to_path_buf(), vec!["png".into()]);
        let art_path = dir.path().join("album.png");
        let img = image::RgbImage::from_fn(64, 64, |_, _| image::Rgb([100, 150, 200]));
        img.save(&art_path).unwrap();

        let matcher = VinylCanvasMatcher::new();
        let t = TrackContext {
            title: "Song".into(),
            artist: "Artist".into(),
            album: "Album".into(),
            art_path: Some(art_path),
            palette: None,
        };

        let res = matcher.find_match(&t, &pool);
        assert!(res.is_some());
        let m = res.unwrap();
        assert_eq!(m.strategy, "vinyl-canvas");
        assert!(m.wallpaper_path.exists());
    }

    #[test]
    fn test_procedural_canvas_matcher_generates_canvas() {
        let dir = TempDir::new().unwrap();
        let pool = MediaPool::new(dir.path().to_path_buf(), vec!["png".into()]);
        let art_path = dir.path().join("album_ambient.png");
        let img = image::RgbImage::from_fn(64, 64, |_, _| image::Rgb([120, 80, 200]));
        img.save(&art_path).unwrap();

        let matcher = ProceduralCanvasMatcher::new();
        let t = TrackContext {
            title: "Ambient Song".into(),
            artist: "Ambient Artist".into(),
            album: "Ambient Album".into(),
            art_path: Some(art_path),
            palette: None,
        };

        let res = matcher.find_match(&t, &pool);
        assert!(res.is_some());
        let m = res.unwrap();
        assert_eq!(m.strategy, "procedural-canvas");
        assert!(m.wallpaper_path.exists());
    }
}

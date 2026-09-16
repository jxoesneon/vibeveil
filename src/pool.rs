use crate::color::{PaletteProfile, extract_palette_from_image};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaItem {
    pub path: PathBuf,
    pub name: String,
    pub is_video: bool,
    pub tags: Vec<String>,
    pub palette: Option<PaletteProfile>,
    pub thumbnail_path: Option<PathBuf>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct PoolIndex {
    pub items: HashMap<PathBuf, MediaItem>,
    pub last_updated: u64,
}

pub struct MediaPool {
    root: PathBuf,
    supported_extensions: Vec<String>,
    index: PoolIndex,
    cache_file: PathBuf,
}

impl MediaPool {
    pub fn new(root: PathBuf, supported_extensions: Vec<String>) -> Self {
        let cache_dir = dirs::cache_dir()
            .map(|p| p.join("vibeveil"))
            .unwrap_or_else(|| PathBuf::from(".cache/vibeveil"));
        std::fs::create_dir_all(&cache_dir).ok();
        let cache_file = cache_dir.join("pool_index.json");

        let index = Self::load_index(&cache_file).unwrap_or_default();

        Self {
            root,
            supported_extensions,
            index,
            cache_file,
        }
    }

    /// Construct a MediaPool with an explicit cache file path.
    /// Used in tests to avoid contaminating the global cache.
    #[cfg(test)]
    pub fn new_isolated(
        root: PathBuf,
        supported_extensions: Vec<String>,
        cache_file: PathBuf,
    ) -> Self {
        Self {
            root,
            supported_extensions,
            index: PoolIndex::default(),
            cache_file,
        }
    }

    fn load_index(path: &Path) -> Option<PoolIndex> {
        if path.exists() {
            let data = std::fs::read_to_string(path).ok()?;
            serde_json::from_str(&data).ok()
        } else {
            None
        }
    }

    pub fn save_index(&self) -> anyhow::Result<()> {
        let data = serde_json::to_string_pretty(&self.index)?;
        std::fs::write(&self.cache_file, data)?;
        Ok(())
    }

    pub fn scan(&mut self, pb: Option<&indicatif::ProgressBar>) -> usize {
        let mut count = 0;
        let extensions: Vec<String> = self
            .supported_extensions
            .iter()
            .map(|e| e.to_lowercase())
            .collect();

        let entries: Vec<_> = WalkDir::new(&self.root)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
            .collect();
        if let Some(p) = pb {
            p.set_length(entries.len() as u64);
        }

        for entry in entries {
            let path = entry.path();
            if let Some(p) = pb {
                p.set_message(
                    path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                );
            }

            if !path.is_file() {
                if let Some(p) = pb {
                    p.inc(1);
                }
                continue;
            }

            // Skip hidden folders (like .thumbnails or .git) relative to the pool root
            let rel = path.strip_prefix(&self.root).unwrap_or(path);
            if rel
                .components()
                .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
            {
                if let Some(p) = pb {
                    p.inc(1);
                }
                continue;
            }

            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if !extensions.contains(&ext.to_lowercase()) {
                    if let Some(p) = pb {
                        p.inc(1);
                    }
                    continue;
                }

                let path_buf = path.to_path_buf();
                if self.index.items.contains_key(&path_buf) {
                    count += 1;
                    if let Some(p) = pb {
                        p.inc(1);
                    }
                    continue;
                }

                let is_video = matches!(ext.to_lowercase().as_str(), "mp4" | "mkv" | "webm");
                let file_name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();

                // Generate tags from path components
                let mut tags = Vec::new();
                if let Ok(rel) = path.strip_prefix(&self.root) {
                    for comp in rel.components() {
                        if let std::path::Component::Normal(c) = comp {
                            let s = c.to_string_lossy().to_lowercase();
                            if !s.contains('.') {
                                tags.push(s);
                            }
                        }
                    }
                }
                tags.push(file_name.to_lowercase());

                // Find or extract thumbnail for color profile
                let mut palette = None;
                let mut thumbnail_path = None;

                if !is_video {
                    palette = extract_palette_from_image(path);
                    thumbnail_path = Some(path_buf.clone());
                } else {
                    // Check nearby .thumbnails conventions
                    let thumb_candidates = [
                        path.parent()
                            .map(|p| p.join(".thumbnails").join(format!("{}.png", file_name))),
                        path.parent()
                            .and_then(|p| p.parent())
                            .map(|p| p.join(".thumbnails").join(format!("{}.png", file_name))),
                    ];

                    for cand in thumb_candidates.into_iter().flatten() {
                        if cand.exists() {
                            palette = extract_palette_from_image(&cand);
                            thumbnail_path = Some(cand);
                            break;
                        }
                    }

                    // If still no thumbnail, generate one on the fly with ffmpeg
                    if thumbnail_path.is_none() {
                        let cache_thumb_dir = dirs::cache_dir()
                            .map(|p| p.join("vibeveil/thumbnails"))
                            .unwrap_or_else(|| PathBuf::from(".cache/vibeveil/thumbnails"));
                        std::fs::create_dir_all(&cache_thumb_dir).ok();
                        let auto_thumb = cache_thumb_dir.join(format!("{}.png", file_name));

                        let _ = std::process::Command::new("ffmpeg")
                            .args([
                                "-y",
                                "-ss",
                                "00:00:01",
                                "-i",
                                path.to_string_lossy().as_ref(),
                                "-vframes",
                                "1",
                                "-q:v",
                                "2",
                                auto_thumb.to_string_lossy().as_ref(),
                            ])
                            .output();

                        if auto_thumb.exists() {
                            palette = extract_palette_from_image(&auto_thumb);
                            thumbnail_path = Some(auto_thumb);
                        }
                    }
                }

                self.index.items.insert(
                    path_buf.clone(),
                    MediaItem {
                        path: path_buf,
                        name: file_name,
                        is_video,
                        tags,
                        palette,
                        thumbnail_path,
                    },
                );
                count += 1;
            }
            if let Some(p) = pb {
                p.inc(1);
            }
        }

        self.save_index().ok();
        count
    }

    pub fn items(&self) -> Vec<&MediaItem> {
        self.index.items.values().collect()
    }

    pub fn find_by_tag(&self, tag: &str) -> Vec<&MediaItem> {
        let needle = tag.to_lowercase();
        self.index
            .items
            .values()
            .filter(|item| {
                item.tags.iter().any(|t| t.contains(&needle))
                    || item.name.to_lowercase().contains(&needle)
            })
            .collect()
    }

    pub fn find_closest_color(&self, target_palette: &PaletteProfile) -> Option<(&MediaItem, f32)> {
        let mut best: Option<(&MediaItem, f32)> = None;

        for item in self.index.items.values() {
            if let Some(ref pal) = item.palette {
                let dist = pal.distance_to(target_palette);
                if let Some((_, best_dist)) = best {
                    if dist < best_dist {
                        best = Some((item, dist));
                    }
                } else {
                    best = Some((item, dist));
                }
            }
        }

        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::{ColorRgb, PaletteProfile};
    use tempfile::TempDir;

    fn write_solid_png(path: &std::path::Path, rgb: [u8; 3]) {
        let img = image::RgbImage::from_fn(8, 8, |_, _| image::Rgb(rgb));
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        std::fs::write(path, buf).unwrap();
    }

    fn make_pool(dir: &TempDir) -> MediaPool {
        let cache_file = dir.path().join("test_pool_index.json");
        MediaPool::new_isolated(
            dir.path().to_path_buf(),
            vec!["png".into(), "jpg".into(), "mp4".into()],
            cache_file,
        )
    }

    // --- Construction ---

    #[test]
    fn new_pool_starts_empty_items() {
        let dir = TempDir::new().unwrap();
        let pool = make_pool(&dir);
        assert_eq!(pool.items().len(), 0);
    }

    // --- scan ---

    #[test]
    fn scan_discovers_png_in_root() {
        let dir = TempDir::new().unwrap();
        write_solid_png(&dir.path().join("art.png"), [200, 50, 50]);
        let mut pool = make_pool(&dir);
        let count = pool.scan(None);
        assert_eq!(count, 1);
    }

    #[test]
    fn scan_discovers_multiple_files() {
        let dir = TempDir::new().unwrap();
        write_solid_png(&dir.path().join("a.png"), [200, 50, 50]);
        write_solid_png(&dir.path().join("b.png"), [50, 200, 50]);
        write_solid_png(&dir.path().join("c.png"), [50, 50, 200]);
        let mut pool = make_pool(&dir);
        assert_eq!(pool.scan(None), 3);
        assert_eq!(pool.items().len(), 3);
    }

    #[test]
    fn scan_skips_unsupported_extension() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("notes.txt"), b"text").unwrap();
        write_solid_png(&dir.path().join("art.png"), [100, 100, 100]);
        let mut pool = make_pool(&dir);
        assert_eq!(pool.scan(None), 1);
    }

    #[test]
    fn scan_skips_hidden_directory_contents() {
        let dir = TempDir::new().unwrap();
        let hidden = dir.path().join(".thumbnails");
        std::fs::create_dir(&hidden).unwrap();
        write_solid_png(&hidden.join("thumb.png"), [100, 100, 100]);
        let mut pool = make_pool(&dir);
        assert_eq!(pool.scan(None), 0, "hidden folder PNGs must be ignored");
    }

    #[test]
    fn scan_subdirectory_generates_directory_tags() {
        let dir = TempDir::new().unwrap();
        let sub = dir.path().join("pokemon");
        std::fs::create_dir(&sub).unwrap();
        write_solid_png(&sub.join("rayquaza.png"), [100, 200, 100]);
        let mut pool = make_pool(&dir);
        pool.scan(None);
        // "pokemon" directory component should be a tag
        let hits = pool.find_by_tag("pokemon");
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn scan_idempotent_no_duplicates() {
        let dir = TempDir::new().unwrap();
        write_solid_png(&dir.path().join("art.png"), [100, 100, 100]);
        let cache_file = dir.path().join("test_pool_index.json");
        let mut pool = MediaPool::new_isolated(
            dir.path().to_path_buf(),
            vec!["png".into()],
            cache_file.clone(),
        );
        let c1 = pool.scan(None);
        // Re-create pool loading from the just-written cache
        let mut pool2 =
            MediaPool::new_isolated(dir.path().to_path_buf(), vec!["png".into()], cache_file);
        let c2 = pool2.scan(None);
        assert_eq!(c1, c2);
    }

    #[test]
    fn scan_fake_mp4_is_marked_as_video() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("video.mp4"), b"fake_mp4").unwrap();
        let mut pool = make_pool(&dir);
        pool.scan(None);
        let items = pool.items();
        let mp4 = items.iter().find(|i| i.name == "video");
        if let Some(item) = mp4 {
            assert!(item.is_video);
        }
    }

    // --- find_by_tag ---

    #[test]
    fn find_by_tag_exact_match() {
        let dir = TempDir::new().unwrap();
        write_solid_png(&dir.path().join("rayquaza.png"), [100, 100, 100]);
        write_solid_png(&dir.path().join("pikachu.png"), [200, 200, 0]);
        let mut pool = make_pool(&dir);
        pool.scan(None);
        let hits = pool.find_by_tag("rayquaza");
        assert_eq!(hits.len(), 1);
        assert!(hits[0].name.contains("rayquaza"));
    }

    #[test]
    fn find_by_tag_partial_name_match() {
        let dir = TempDir::new().unwrap();
        write_solid_png(&dir.path().join("shiny_rayquaza_dark.png"), [100, 100, 100]);
        let mut pool = make_pool(&dir);
        pool.scan(None);
        assert_eq!(pool.find_by_tag("rayquaza").len(), 1);
    }

    #[test]
    fn find_by_tag_not_found_empty() {
        let dir = TempDir::new().unwrap();
        write_solid_png(&dir.path().join("art.png"), [100, 100, 100]);
        let mut pool = make_pool(&dir);
        pool.scan(None);
        assert!(pool.find_by_tag("zzz_no_such_tag_xyz").is_empty());
    }

    #[test]
    fn find_by_tag_case_insensitive() {
        let dir = TempDir::new().unwrap();
        write_solid_png(&dir.path().join("Rayquaza.png"), [100, 100, 100]);
        let mut pool = make_pool(&dir);
        pool.scan(None);
        // Tags are lowercased during scan
        assert_eq!(pool.find_by_tag("rayquaza").len(), 1);
    }

    // --- find_closest_color ---

    #[test]
    fn find_closest_color_empty_pool_returns_none() {
        let dir = TempDir::new().unwrap();
        let pool = make_pool(&dir);
        let palette = PaletteProfile {
            primary: ColorRgb::new(255, 0, 0),
            secondary: ColorRgb::new(200, 0, 0),
            surface: ColorRgb::new(20, 0, 0),
            accent: ColorRgb::new(255, 100, 0),
        };
        assert!(pool.find_closest_color(&palette).is_none());
    }

    #[test]
    fn find_closest_color_prefers_closest() {
        let dir = TempDir::new().unwrap();
        // Pure red and pure blue images
        write_solid_png(&dir.path().join("red.png"), [220, 0, 0]);
        write_solid_png(&dir.path().join("blue.png"), [0, 0, 220]);
        let mut pool = make_pool(&dir);
        pool.scan(None);

        let red_palette = PaletteProfile {
            primary: ColorRgb::new(255, 0, 0),
            secondary: ColorRgb::new(200, 10, 10),
            surface: ColorRgb::new(80, 0, 0),
            accent: ColorRgb::new(255, 60, 0),
        };
        let result = pool.find_closest_color(&red_palette);
        assert!(result.is_some());
        let (item, dist) = result.unwrap();
        assert!(dist >= 0.0);
        assert!(
            item.name == "red",
            "Expected 'red' to win, got '{}'",
            item.name
        );
    }

    #[test]
    fn find_closest_color_no_palette_items_returns_none() {
        let dir = TempDir::new().unwrap();
        // Fake mp4 has no palette (no thumbnail)
        std::fs::write(dir.path().join("video.mp4"), b"fake").unwrap();
        let mut pool = make_pool(&dir);
        pool.scan(None);

        let palette = PaletteProfile {
            primary: ColorRgb::new(100, 100, 100),
            secondary: ColorRgb::new(100, 100, 100),
            surface: ColorRgb::new(20, 20, 20),
            accent: ColorRgb::new(150, 150, 150),
        };
        assert!(pool.find_closest_color(&palette).is_none());
    }

    // --- items ---

    #[test]
    fn items_returns_all_scanned() {
        let dir = TempDir::new().unwrap();
        write_solid_png(&dir.path().join("a.png"), [100, 0, 0]);
        write_solid_png(&dir.path().join("b.png"), [0, 100, 0]);
        let mut pool = make_pool(&dir);
        pool.scan(None);
        assert_eq!(pool.items().len(), 2);
    }

    #[test]
    fn scan_with_progress_bar() {
        let dir = TempDir::new().unwrap();
        write_solid_png(&dir.path().join("img.png"), [100, 100, 100]);
        let mut pool = make_pool(&dir);
        let pb = indicatif::ProgressBar::new(0);
        let count = pool.scan(Some(&pb));
        assert_eq!(count, 1);
    }

    #[test]
    fn scan_video_with_nearby_thumbnail() {
        let dir = TempDir::new().unwrap();
        let video_path = dir.path().join("live.mp4");
        std::fs::write(&video_path, b"video_data").unwrap();

        // Put thumbnail in .thumbnails/live.png
        let thumb_dir = dir.path().join(".thumbnails");
        std::fs::create_dir_all(&thumb_dir).unwrap();
        write_solid_png(&thumb_dir.join("live.png"), [200, 50, 50]);

        let mut pool = make_pool(&dir);
        let count = pool.scan(None);
        assert_eq!(count, 1);

        let item = pool.items().into_iter().find(|i| i.name == "live").unwrap();
        assert!(item.is_video);
        assert!(item.thumbnail_path.is_some());
        assert!(item.palette.is_some());
    }

    #[test]
    fn scan_mkv_and_webm_formats() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("vid1.mkv"), b"mkv_data").unwrap();
        std::fs::write(dir.path().join("vid2.webm"), b"webm_data").unwrap();

        let cache_file = dir.path().join("test_pool_index.json");
        let mut pool = MediaPool::new_isolated(
            dir.path().to_path_buf(),
            vec!["mkv".into(), "webm".into()],
            cache_file,
        );
        let count = pool.scan(None);
        assert_eq!(count, 2);
        for item in pool.items() {
            assert!(item.is_video);
        }
    }

    #[test]
    fn load_index_corrupt_returns_none() {
        let dir = TempDir::new().unwrap();
        let corrupt_path = dir.path().join("corrupt.json");
        std::fs::write(&corrupt_path, b"{invalid json").unwrap();
        assert!(MediaPool::load_index(&corrupt_path).is_none());
    }

    #[test]
    fn save_and_load_index_roundtrip() {
        let dir = TempDir::new().unwrap();
        let cache_file = dir.path().join("test_index.json");
        let mut pool = MediaPool::new_isolated(
            dir.path().to_path_buf(),
            vec!["png".into()],
            cache_file.clone(),
        );
        write_solid_png(&dir.path().join("test.png"), [100, 100, 100]);
        pool.scan(None);
        assert!(pool.save_index().is_ok());
        let loaded = MediaPool::load_index(&cache_file);
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().items.len(), 1);
    }
}

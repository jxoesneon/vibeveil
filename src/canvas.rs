use anyhow::Result;
use image::imageops;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub struct CanvasGenerator {
    cache_dir: PathBuf,
}

impl CanvasGenerator {
    pub fn new() -> Self {
        let cache_dir = dirs::cache_dir()
            .map(|p| p.join("vibeveil/album_art"))
            .unwrap_or_else(|| PathBuf::from(".cache/vibeveil/album_art"));
        std::fs::create_dir_all(&cache_dir).ok();
        Self { cache_dir }
    }

    pub async fn fetch_or_cache_art(&self, url: &str) -> Result<PathBuf> {
        if url.starts_with("file://") {
            return Ok(PathBuf::from(url.trim_start_matches("file://")));
        }

        let mut hasher = Sha256::new();
        hasher.update(url.as_bytes());
        let hash = format!("{:x}", hasher.finalize());
        let cached_path = self.cache_dir.join(format!("{}.png", hash));

        if cached_path.exists() {
            return Ok(cached_path);
        }

        let bytes = reqwest::get(url).await?.bytes().await?;
        let img = image::load_from_memory(&bytes)?;
        img.save(&cached_path)?;

        Ok(cached_path)
    }

    pub fn generate_ambient_canvas(
        &self,
        art_path: &Path,
        width: u32,
        height: u32,
    ) -> Result<PathBuf> {
        let art = image::open(art_path)?;
        let out_path = self.cache_dir.join("current_ambient_canvas.png");

        // 1. Create blurred ambient background
        let bg_small = art.resize_exact(80, 45, imageops::FilterType::Nearest);
        let bg_blurred = bg_small.blur(8.0);
        let mut bg_full = bg_blurred
            .resize_exact(width, height, imageops::FilterType::Triangle)
            .to_rgba8();

        // Darken background vignette slightly for window contrast
        for pixel in bg_full.pixels_mut() {
            pixel[0] = (pixel[0] as f32 * 0.45) as u8;
            pixel[1] = (pixel[1] as f32 * 0.45) as u8;
            pixel[2] = (pixel[2] as f32 * 0.45) as u8;
        }

        // 2. Center-floating crisp album art card
        let card_size = (height as f32 * 0.58) as u32;
        let card = art.resize_exact(card_size, card_size, imageops::FilterType::Lanczos3);

        let pos_x = (width - card_size) / 2;
        let pos_y = (height - card_size) / 2;

        // Overlay with subtle shadow border
        imageops::overlay(&mut bg_full, &card, pos_x as i64, pos_y as i64);

        bg_full.save(&out_path)?;
        Ok(out_path)
    }

    /// Procedurally synthesizes a high-definition spinning vinyl disc canvas,
    /// complete with concentric grooves, specular sheen highlights, and a circular album art label.
    pub fn generate_vinyl_canvas(
        &self,
        art_path: &Path,
        width: u32,
        height: u32,
    ) -> Result<PathBuf> {
        let art = image::open(art_path)?;
        let out_path = self.cache_dir.join("current_vinyl_canvas.png");

        // 1. Create dark ambient blurred background
        let bg_small = art.resize_exact(80, 45, imageops::FilterType::Nearest);
        let bg_blurred = bg_small.blur(10.0);
        let mut canvas = bg_blurred
            .resize_exact(width, height, imageops::FilterType::Triangle)
            .to_rgba8();

        for pixel in canvas.pixels_mut() {
            pixel[0] = (pixel[0] as f32 * 0.35) as u8;
            pixel[1] = (pixel[1] as f32 * 0.35) as u8;
            pixel[2] = (pixel[2] as f32 * 0.35) as u8;
        }

        // 2. Compute vinyl disc dimensions
        let cx = width as f32 / 2.0;
        let cy = height as f32 / 2.0;
        let disc_radius = (height as f32 * 0.42).min(width as f32 * 0.42);
        let label_radius = disc_radius * 0.38;
        let spindle_radius = disc_radius * 0.045;

        // Resize album art for the circular label
        let label_diam = (label_radius * 2.0) as u32;
        let label_art = art
            .resize_exact(label_diam, label_diam, imageops::FilterType::Lanczos3)
            .to_rgba8();

        let min_x = ((cx - disc_radius).max(0.0)) as u32;
        let max_x = ((cx + disc_radius).min(width as f32 - 1.0)) as u32;
        let min_y = ((cy - disc_radius).max(0.0)) as u32;
        let max_y = ((cy + disc_radius).min(height as f32 - 1.0)) as u32;

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                let dist = (dx * dx + dy * dy).sqrt();

                if dist > disc_radius {
                    continue;
                }

                if dist <= spindle_radius {
                    // Spindle hole
                    canvas.put_pixel(x, y, image::Rgba([18, 18, 18, 255]));
                } else if dist <= label_radius {
                    // Center circular album art label
                    let lx = (dx + label_radius) as u32;
                    let ly = (dy + label_radius) as u32;
                    if lx < label_diam && ly < label_diam {
                        let p = label_art.get_pixel(lx, ly);
                        canvas.put_pixel(x, y, *p);
                    }
                } else {
                    // Vinyl grooves: dark base with sinusoidal micro-groove reflections
                    let groove_val = ((dist * 1.8).sin() * 12.0) as i16;
                    let angle = dy.atan2(dx);
                    let sheen = ((angle * 2.0).sin().abs() * 22.0) as i16;

                    let base = 20 + groove_val + sheen;
                    let val = base.clamp(10, 85) as u8;
                    canvas.put_pixel(
                        x,
                        y,
                        image::Rgba([val, val, (val as f32 * 1.05).min(255.0) as u8, 255]),
                    );
                }
            }
        }

        canvas.save(&out_path)?;
        Ok(out_path)
    }

    fn vinyl_hash(&self, art_path: &Path) -> String {
        let mut hasher = Sha256::new();
        hasher.update(art_path.to_string_lossy().as_bytes());
        if let Ok(metadata) = std::fs::metadata(art_path) {
            hasher.update(metadata.len().to_le_bytes());
            if let Ok(mtime) = metadata.modified()
                && let Ok(dur) = mtime.duration_since(std::time::UNIX_EPOCH)
            {
                hasher.update(dur.as_secs().to_le_bytes());
            }
        }
        format!("{:x}", hasher.finalize())
    }

    pub fn cached_vinyl_video_loop(&self, art_path: &Path) -> Option<PathBuf> {
        let hash = self.vinyl_hash(art_path);
        let out_mp4 = self.cache_dir.join(format!("{}_vinyl.mp4", hash));
        if out_mp4.exists()
            && std::fs::metadata(&out_mp4)
                .map(|m| m.len() > 1000)
                .unwrap_or(false)
        {
            Some(out_mp4)
        } else {
            None
        }
    }

    pub fn cached_ambient_video_loop(&self, art_path: &Path) -> Option<PathBuf> {
        let hash = self.vinyl_hash(art_path);
        let out_mp4 = self.cache_dir.join(format!("{}_ambient.mp4", hash));
        if out_mp4.exists()
            && std::fs::metadata(&out_mp4)
                .map(|m| m.len() > 1000)
                .unwrap_or(false)
        {
            Some(out_mp4)
        } else {
            None
        }
    }

    pub fn generate_vinyl_poster(
        &self,
        art_path: &Path,
        width: u32,
        height: u32,
    ) -> Result<PathBuf> {
        let hash = self.vinyl_hash(art_path);
        let out_png = self.cache_dir.join(format!("{}_vinyl.png", hash));
        if out_png.exists() {
            return Ok(out_png);
        }

        let art = image::open(art_path)?;

        let bg_small = art.resize_exact(80, 45, imageops::FilterType::Nearest);
        let bg_blurred = bg_small.blur(10.0);
        let mut bg_canvas = bg_blurred
            .resize_exact(width, height, imageops::FilterType::Triangle)
            .to_rgba8();

        for pixel in bg_canvas.pixels_mut() {
            pixel[0] = (pixel[0] as f32 * 0.35) as u8;
            pixel[1] = (pixel[1] as f32 * 0.35) as u8;
            pixel[2] = (pixel[2] as f32 * 0.35) as u8;
        }

        let disc_radius = (height as f32 * 0.42).min(width as f32 * 0.42);
        let disc_dim = ((disc_radius * 2.0).ceil() as u32).max(100);
        let cx = disc_dim as f32 / 2.0;
        let cy = disc_dim as f32 / 2.0;
        let label_radius = disc_radius * 0.38;
        let spindle_radius = disc_radius * 0.045;

        let label_diam = (label_radius * 2.0) as u32;
        let label_art = art
            .resize_exact(label_diam, label_diam, imageops::FilterType::Lanczos3)
            .to_rgba8();

        let mut disc_canvas = image::RgbaImage::new(disc_dim, disc_dim);

        for y in 0..disc_dim {
            for x in 0..disc_dim {
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                let dist = (dx * dx + dy * dy).sqrt();

                if dist > disc_radius {
                    continue;
                }

                if dist <= spindle_radius {
                    disc_canvas.put_pixel(x, y, image::Rgba([18, 18, 18, 255]));
                } else if dist <= label_radius {
                    let lx = (dx + label_radius) as u32;
                    let ly = (dy + label_radius) as u32;
                    if lx < label_diam && ly < label_diam {
                        let p = label_art.get_pixel(lx, ly);
                        disc_canvas.put_pixel(x, y, *p);
                    }
                } else {
                    let groove_val = ((dist * 1.8).sin() * 12.0) as i16;
                    let angle = dy.atan2(dx);
                    let sheen = ((angle * 2.0).sin().abs() * 22.0) as i16;

                    let base = 20 + groove_val + sheen;
                    let val = base.clamp(10, 85) as u8;
                    disc_canvas.put_pixel(
                        x,
                        y,
                        image::Rgba([val, val, (val as f32 * 1.05).min(255.0) as u8, 255]),
                    );
                }
            }
        }

        let mut poster = bg_canvas;
        let pos_x = (width.saturating_sub(disc_dim)) / 2;
        let pos_y = (height.saturating_sub(disc_dim)) / 2;
        imageops::overlay(&mut poster, &disc_canvas, pos_x as i64, pos_y as i64);
        poster.save(&out_png)?;

        Ok(out_png)
    }

    pub fn generate_ambient_poster(
        &self,
        art_path: &Path,
        width: u32,
        height: u32,
    ) -> Result<PathBuf> {
        let hash = self.vinyl_hash(art_path);
        let out_png = self.cache_dir.join(format!("{}_ambient.png", hash));
        if out_png.exists() {
            return Ok(out_png);
        }

        let art = image::open(art_path)?;
        let bg_small = art.resize_exact(80, 45, imageops::FilterType::Nearest);
        let bg_blurred = bg_small.blur(8.0);
        let mut bg_canvas = bg_blurred
            .resize_exact(width, height, imageops::FilterType::Triangle)
            .to_rgba8();

        for pixel in bg_canvas.pixels_mut() {
            pixel[0] = (pixel[0] as f32 * 0.45) as u8;
            pixel[1] = (pixel[1] as f32 * 0.45) as u8;
            pixel[2] = (pixel[2] as f32 * 0.45) as u8;
        }

        let card_size = (height as f32 * 0.58) as u32;
        let card = art.resize_exact(card_size, card_size, imageops::FilterType::Lanczos3);

        let mut poster = bg_canvas;
        let pos_x = (width.saturating_sub(card_size)) / 2;
        let pos_y = (height.saturating_sub(card_size)) / 2;
        imageops::overlay(&mut poster, &card, pos_x as i64, pos_y as i64);
        poster.save(&out_png)?;

        Ok(out_png)
    }

    pub fn generate_vinyl_video_loop(
        &self,
        art_path: &Path,
        width: u32,
        height: u32,
    ) -> Result<PathBuf> {
        let hash = self.vinyl_hash(art_path);
        let out_mp4 = self.cache_dir.join(format!("{}_vinyl.mp4", hash));

        if out_mp4.exists()
            && std::fs::metadata(&out_mp4)
                .map(|m| m.len() > 1000)
                .unwrap_or(false)
        {
            return Ok(out_mp4);
        }

        let _ = self.generate_vinyl_poster(art_path, width, height)?;

        let art = image::open(art_path)?;
        let bg_small = art.resize_exact(80, 45, imageops::FilterType::Nearest);
        let bg_blurred = bg_small.blur(10.0);
        let mut bg_canvas = bg_blurred
            .resize_exact(width, height, imageops::FilterType::Triangle)
            .to_rgba8();

        for pixel in bg_canvas.pixels_mut() {
            pixel[0] = (pixel[0] as f32 * 0.35) as u8;
            pixel[1] = (pixel[1] as f32 * 0.35) as u8;
            pixel[2] = (pixel[2] as f32 * 0.35) as u8;
        }

        let disc_radius = (height as f32 * 0.42).min(width as f32 * 0.42);
        let disc_dim = ((disc_radius * 2.0).ceil() as u32).max(100);
        let cx = disc_dim as f32 / 2.0;
        let cy = disc_dim as f32 / 2.0;
        let label_radius = disc_radius * 0.38;
        let spindle_radius = disc_radius * 0.045;

        let label_diam = (label_radius * 2.0) as u32;
        let label_art = art
            .resize_exact(label_diam, label_diam, imageops::FilterType::Lanczos3)
            .to_rgba8();

        let mut disc_canvas = image::RgbaImage::new(disc_dim, disc_dim);

        for y in 0..disc_dim {
            for x in 0..disc_dim {
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                let dist = (dx * dx + dy * dy).sqrt();

                if dist > disc_radius {
                    continue;
                }

                if dist <= spindle_radius {
                    disc_canvas.put_pixel(x, y, image::Rgba([18, 18, 18, 255]));
                } else if dist <= label_radius {
                    let lx = (dx + label_radius) as u32;
                    let ly = (dy + label_radius) as u32;
                    if lx < label_diam && ly < label_diam {
                        let p = label_art.get_pixel(lx, ly);
                        disc_canvas.put_pixel(x, y, *p);
                    }
                } else {
                    let groove_val = ((dist * 1.8).sin() * 12.0) as i16;
                    let angle = dy.atan2(dx);
                    let sheen = ((angle * 2.0).sin().abs() * 22.0) as i16;

                    let base = 20 + groove_val + sheen;
                    let val = base.clamp(10, 85) as u8;
                    disc_canvas.put_pixel(
                        x,
                        y,
                        image::Rgba([val, val, (val as f32 * 1.05).min(255.0) as u8, 255]),
                    );
                }
            }
        }

        let bg_tmp = self.cache_dir.join(format!("{}_bg_tmp.png", hash));
        let disc_tmp = self.cache_dir.join(format!("{}_disc_tmp.png", hash));
        bg_canvas.save(&bg_tmp)?;
        disc_canvas.save(&disc_tmp)?;

        let output = std::process::Command::new("ffmpeg")
            .args([
                "-y",
                "-loop",
                "1",
                "-framerate",
                "30",
                "-t",
                "2",
                "-i",
                bg_tmp.to_string_lossy().as_ref(),
                "-loop",
                "1",
                "-framerate",
                "30",
                "-t",
                "2",
                "-i",
                disc_tmp.to_string_lossy().as_ref(),
                "-filter_complex",
                "[1:v]format=rgba,rotate=2*PI*t/2:c=none:ow=iw:oh=ih[rot];[0:v][rot]overlay=(W-w)/2:(H-h)/2:shortest=1[outv]",
                "-map",
                "[outv]",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-preset",
                "ultrafast",
                "-threads",
                "0",
                "-r",
                "30",
                out_mp4.to_string_lossy().as_ref(),
            ])
            .output();

        let _ = std::fs::remove_file(bg_tmp);
        let _ = std::fs::remove_file(disc_tmp);

        match output {
            Ok(res) if res.status.success() && out_mp4.exists() => Ok(out_mp4),
            Ok(res) => anyhow::bail!("FFmpeg failed with exit code: {:?}", res.status.code()),
            Err(e) => anyhow::bail!("Failed to execute FFmpeg: {}", e),
        }
    }

    pub fn generate_ambient_video_loop(
        &self,
        art_path: &Path,
        width: u32,
        height: u32,
    ) -> Result<PathBuf> {
        let hash = self.vinyl_hash(art_path);
        let out_mp4 = self.cache_dir.join(format!("{}_ambient.mp4", hash));

        if out_mp4.exists()
            && std::fs::metadata(&out_mp4)
                .map(|m| m.len() > 1000)
                .unwrap_or(false)
        {
            return Ok(out_mp4);
        }

        let _ = self.generate_ambient_poster(art_path, width, height)?;

        let art = image::open(art_path)?;
        let bg_small = art.resize_exact(80, 45, imageops::FilterType::Nearest);
        let bg_blurred = bg_small.blur(8.0);
        let mut bg_canvas = bg_blurred
            .resize_exact(width, height, imageops::FilterType::Triangle)
            .to_rgba8();

        for pixel in bg_canvas.pixels_mut() {
            pixel[0] = (pixel[0] as f32 * 0.45) as u8;
            pixel[1] = (pixel[1] as f32 * 0.45) as u8;
            pixel[2] = (pixel[2] as f32 * 0.45) as u8;
        }

        let card_size = (height as f32 * 0.58) as u32;
        let card = art.resize_exact(card_size, card_size, imageops::FilterType::Lanczos3);

        let bg_tmp = self.cache_dir.join(format!("{}_amb_bg_tmp.png", hash));
        let card_tmp = self.cache_dir.join(format!("{}_amb_card_tmp.png", hash));
        bg_canvas.save(&bg_tmp)?;
        card.save(&card_tmp)?;

        let output = std::process::Command::new("ffmpeg")
            .args([
                "-y",
                "-loop",
                "1",
                "-framerate",
                "24",
                "-t",
                "2",
                "-i",
                bg_tmp.to_string_lossy().as_ref(),
                "-loop",
                "1",
                "-framerate",
                "24",
                "-t",
                "2",
                "-i",
                card_tmp.to_string_lossy().as_ref(),
                "-filter_complex",
                "[1:v]format=rgba[c];[0:v][c]overlay=(W-w)/2:'(H-h)/2+8*sin(2*PI*t/2)':shortest=1[outv]",
                "-map",
                "[outv]",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-preset",
                "ultrafast",
                "-tune",
                "fastdecode",
                "-threads",
                "0",
                "-r",
                "24",
                out_mp4.to_string_lossy().as_ref(),
            ])
            .output();

        let _ = std::fs::remove_file(bg_tmp);
        let _ = std::fs::remove_file(card_tmp);

        match output {
            Ok(res) if res.status.success() && out_mp4.exists() => Ok(out_mp4),
            Ok(res) => {
                anyhow::bail!(
                    "FFmpeg ambient failed with exit code: {:?}",
                    res.status.code()
                )
            }
            Err(e) => anyhow::bail!("Failed to execute FFmpeg: {}", e),
        }
    }
}

pub async fn prune_lru_cache(cache_dir: &Path, max_mb: u64) {
    let max_bytes = max_mb * 1024 * 1024;
    let mut files = vec![];
    let mut total_size = 0;

    if let Ok(mut entries) = tokio::fs::read_dir(cache_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            if let Ok(metadata) = entry.metadata().await
                && metadata.is_file()
            {
                let size = metadata.len();
                total_size += size;
                let modified = metadata
                    .modified()
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                files.push((entry.path(), size, modified));
            }
        }
    }

    if total_size > max_bytes {
        files.sort_by_key(|(_, _, modified)| *modified);
        for (path, size, _) in files {
            if tokio::fs::remove_file(&path).await.is_ok() {
                total_size = total_size.saturating_sub(size);
                if total_size <= max_bytes {
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_solid_png(path: &Path, rgb: [u8; 3]) {
        let img = image::RgbImage::from_fn(16, 16, |_, _| image::Rgb(rgb));
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        std::fs::write(path, buf).unwrap();
    }

    fn make_cg(dir: &TempDir) -> CanvasGenerator {
        let cache = dir.path().join("cache");
        std::fs::create_dir_all(&cache).unwrap();
        CanvasGenerator { cache_dir: cache }
    }

    // --- fetch_or_cache_art ---

    #[tokio::test]
    async fn fetch_file_url_returns_stripped_path() {
        let dir = TempDir::new().unwrap();
        let art = dir.path().join("art.png");
        write_solid_png(&art, [200, 100, 50]);

        let cg = make_cg(&dir);
        let url = format!("file://{}", art.to_string_lossy());
        let result = cg.fetch_or_cache_art(&url).await.unwrap();
        assert_eq!(result, art);
    }

    #[tokio::test]
    async fn fetch_file_url_second_call_same_result() {
        let dir = TempDir::new().unwrap();
        let art = dir.path().join("art.png");
        write_solid_png(&art, [100, 100, 200]);
        let cg = make_cg(&dir);
        let url = format!("file://{}", art.to_string_lossy());
        let r1 = cg.fetch_or_cache_art(&url).await.unwrap();
        let r2 = cg.fetch_or_cache_art(&url).await.unwrap();
        assert_eq!(r1, r2);
    }

    // --- generate_ambient_canvas ---

    #[test]
    fn generate_canvas_produces_correct_dimensions() {
        let dir = TempDir::new().unwrap();
        let art = dir.path().join("source.png");
        write_solid_png(&art, [180, 80, 50]);

        let cg = make_cg(&dir);
        let out = cg.generate_ambient_canvas(&art, 1920, 1080).unwrap();
        assert!(out.exists());
        let img = image::open(&out).unwrap();
        assert_eq!(img.width(), 1920);
        assert_eq!(img.height(), 1080);
    }

    #[test]
    fn generate_canvas_640x480() {
        let dir = TempDir::new().unwrap();
        let art = dir.path().join("art.png");
        let src =
            image::RgbImage::from_fn(64, 64, |x, y| image::Rgb([x as u8 * 4, y as u8 * 4, 128u8]));
        let mut buf = Vec::new();
        src.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        std::fs::write(&art, buf).unwrap();

        let cg = make_cg(&dir);
        let out = cg.generate_ambient_canvas(&art, 640, 480).unwrap();
        assert!(image::open(out).is_ok());
    }

    #[test]
    fn generate_canvas_invalid_source_returns_err() {
        let dir = TempDir::new().unwrap();
        let cg = make_cg(&dir);
        let result = cg.generate_ambient_canvas(Path::new("/nonexistent/art.png"), 1920, 1080);
        assert!(result.is_err());
    }

    #[test]
    fn generate_canvas_small_resolution() {
        let dir = TempDir::new().unwrap();
        let art = dir.path().join("art.png");
        write_solid_png(&art, [100, 200, 100]);
        let cg = make_cg(&dir);
        let out = cg.generate_ambient_canvas(&art, 320, 240).unwrap();
        assert!(out.exists());
        let img = image::open(&out).unwrap();
        assert_eq!((img.width(), img.height()), (320, 240));
    }

    #[test]
    fn generate_canvas_overwrites_previous() {
        let dir = TempDir::new().unwrap();
        let art = dir.path().join("art.png");
        write_solid_png(&art, [200, 0, 0]);
        let cg = make_cg(&dir);
        cg.generate_ambient_canvas(&art, 100, 100).unwrap();
        // Second call should overwrite without error
        assert!(cg.generate_ambient_canvas(&art, 100, 100).is_ok());
    }

    // --- prune_lru_cache ---

    #[tokio::test]
    async fn prune_empty_dir_no_panic() {
        let dir = TempDir::new().unwrap();
        prune_lru_cache(dir.path(), 100).await;
    }

    #[tokio::test]
    async fn prune_nonexistent_dir_no_panic() {
        prune_lru_cache(Path::new("/tmp/vibeveil_test_nonexistent_dir_xyz"), 100).await;
    }

    #[tokio::test]
    async fn prune_under_limit_keeps_files() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("small.dat"), b"tiny").unwrap();
        prune_lru_cache(dir.path(), 100).await; // 100 MB limit — well under
        assert!(dir.path().join("small.dat").exists());
    }

    #[tokio::test]
    async fn prune_over_limit_removes_oldest_file() {
        let dir = TempDir::new().unwrap();
        let old_file = dir.path().join("old.dat");
        let new_file = dir.path().join("new.dat");

        // 600 KB each → 1.2 MB total → over 1 MB limit
        let data = vec![0u8; 600 * 1024];
        std::fs::write(&old_file, &data).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(&new_file, &data).unwrap();

        prune_lru_cache(dir.path(), 1).await; // 1 MB limit

        // Newest file must survive
        assert!(new_file.exists(), "newest file should survive pruning");
    }

    #[tokio::test]
    async fn prune_under_1mb_both_survive() {
        let dir = TempDir::new().unwrap();
        let f1 = dir.path().join("f1.dat");
        let f2 = dir.path().join("f2.dat");
        // 300 KB each → 600 KB total < 1 MB limit
        let data = vec![0u8; 300 * 1024];
        std::fs::write(&f1, &data).unwrap();
        std::fs::write(&f2, &data).unwrap();
        prune_lru_cache(dir.path(), 1).await;
        assert!(f1.exists());
        assert!(f2.exists());
    }

    #[test]
    fn test_generate_vinyl_canvas_produces_image() {
        let dir = TempDir::new().unwrap();
        let art_path = dir.path().join("album.png");
        let img = image::RgbImage::from_fn(128, 128, |x, y| {
            image::Rgb([(x % 255) as u8, (y % 255) as u8, 128])
        });
        img.save(&art_path).unwrap();

        let generator = CanvasGenerator {
            cache_dir: dir.path().to_path_buf(),
        };
        let res = generator.generate_vinyl_canvas(&art_path, 640, 360);
        assert!(res.is_ok());
        let out = res.unwrap();
        assert!(out.exists());

        let loaded = image::open(&out).unwrap();
        assert_eq!(loaded.width(), 640);
        assert_eq!(loaded.height(), 360);
    }

    #[test]
    fn test_generate_vinyl_video_loop_produces_mp4() {
        let ffmpeg_available = std::process::Command::new("which")
            .arg("ffmpeg")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !ffmpeg_available {
            return;
        }

        let dir = TempDir::new().unwrap();
        let art_path = dir.path().join("album_loop.png");
        let img = image::RgbImage::from_fn(128, 128, |x, y| {
            image::Rgb([(x % 255) as u8, (y % 255) as u8, 180])
        });
        img.save(&art_path).unwrap();

        let generator = CanvasGenerator {
            cache_dir: dir.path().to_path_buf(),
        };
        let res = generator.generate_vinyl_video_loop(&art_path, 320, 180);
        assert!(res.is_ok());
        let mp4_path = res.unwrap();
        assert!(mp4_path.exists());
        assert!(mp4_path.extension().and_then(|s| s.to_str()) == Some("mp4"));
        assert!(std::fs::metadata(&mp4_path).unwrap().len() > 1000);

        let poster_png = mp4_path.with_extension("png");
        assert!(poster_png.exists());
        assert!(image::open(poster_png).is_ok());

        // Second call should return cached without regenerating
        let res2 = generator.generate_vinyl_video_loop(&art_path, 320, 180);
        assert_eq!(res2.unwrap(), mp4_path);
    }

    #[test]
    fn test_generate_ambient_video_loop_produces_mp4() {
        let ffmpeg_available = std::process::Command::new("which")
            .arg("ffmpeg")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !ffmpeg_available {
            return;
        }

        let dir = TempDir::new().unwrap();
        let art_path = dir.path().join("ambient_loop.png");
        let img = image::RgbImage::from_fn(128, 128, |x, y| {
            image::Rgb([40, (x % 255) as u8, (y % 255) as u8])
        });
        img.save(&art_path).unwrap();

        let generator = CanvasGenerator {
            cache_dir: dir.path().to_path_buf(),
        };
        let res = generator.generate_ambient_video_loop(&art_path, 320, 180);
        assert!(res.is_ok());
        let mp4_path = res.unwrap();
        assert!(mp4_path.exists());
        assert!(mp4_path.extension().and_then(|s| s.to_str()) == Some("mp4"));
        assert!(std::fs::metadata(&mp4_path).unwrap().len() > 1000);

        let poster_png = mp4_path.with_extension("png");
        assert!(poster_png.exists());
        assert!(image::open(poster_png).is_ok());

        // Second call should return cached without regenerating
        let res2 = generator.generate_ambient_video_loop(&art_path, 320, 180);
        assert_eq!(res2.unwrap(), mp4_path);
    }
}

use palette::color_difference::EuclideanDistance;
use palette::{FromColor, Oklab, Srgb};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColorRgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl ColorRgb {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub fn hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

impl std::fmt::Display for ColorRgb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

impl ColorRgb {
    pub fn to_oklab(self) -> Oklab {
        let srgb = Srgb::new(
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
        );
        Oklab::from_color(srgb)
    }

    pub fn delta_e_oklab(&self, other: &ColorRgb) -> f32 {
        let ok1 = self.to_oklab();
        let ok2 = other.to_oklab();
        ok1.distance(ok2) * 100.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaletteProfile {
    pub primary: ColorRgb,
    pub secondary: ColorRgb,
    pub surface: ColorRgb,
    pub accent: ColorRgb,
}

impl PaletteProfile {
    pub fn distance_to(&self, other: &PaletteProfile) -> f32 {
        // Weighted perceptual color distance
        let d_primary = self.primary.delta_e_oklab(&other.primary) * 0.50;
        let d_secondary = self.secondary.delta_e_oklab(&other.secondary) * 0.30;
        let d_accent = self.accent.delta_e_oklab(&other.accent) * 0.20;
        d_primary + d_secondary + d_accent
    }
}

pub fn extract_palette_from_image(path: &Path) -> Option<PaletteProfile> {
    let img = image::open(path).ok()?;
    let thumb = img.thumbnail(64, 64).to_rgb8();

    let mut samples: Vec<ColorRgb> =
        Vec::with_capacity(thumb.width() as usize * thumb.height() as usize);
    for pixel in thumb.pixels() {
        samples.push(ColorRgb::new(pixel[0], pixel[1], pixel[2]));
    }

    if samples.is_empty() {
        return None;
    }

    // Simple k-means-like quantization for 4 key clusters
    let primary = find_vibrant_dominant(&samples)?;
    let surface = find_darkest_background(&samples).unwrap_or(ColorRgb::new(18, 18, 20));
    let secondary = find_contrasting_accent(&samples, &primary).unwrap_or(primary);
    let accent = find_most_saturated(&samples).unwrap_or(primary);

    Some(PaletteProfile {
        primary,
        secondary,
        surface,
        accent,
    })
}

fn luminance(c: &ColorRgb) -> f32 {
    0.2126 * (c.r as f32) + 0.7152 * (c.g as f32) + 0.0722 * (c.b as f32)
}

fn saturation(c: &ColorRgb) -> f32 {
    let max = c.r.max(c.g).max(c.b) as f32;
    let min = c.r.min(c.g).min(c.b) as f32;
    if max == 0.0 { 0.0 } else { (max - min) / max }
}

fn find_vibrant_dominant(samples: &[ColorRgb]) -> Option<ColorRgb> {
    let mut best_score = -1.0;
    let mut best_color = None;

    // Filter out extreme blacks and whites
    for c in samples.iter().step_by(4) {
        let lum = luminance(c);
        let sat = saturation(c);

        if lum > 25.0 && lum < 235.0 {
            let score = sat * 1.5 + (1.0 - ((lum - 128.0).abs() / 128.0));
            if score > best_score {
                best_score = score;
                best_color = Some(*c);
            }
        }
    }

    best_color.or_else(|| samples.first().copied())
}

fn find_darkest_background(samples: &[ColorRgb]) -> Option<ColorRgb> {
    samples
        .iter()
        .min_by(|a, b| {
            luminance(a)
                .partial_cmp(&luminance(b))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .copied()
}

fn find_contrasting_accent(samples: &[ColorRgb], base: &ColorRgb) -> Option<ColorRgb> {
    let mut max_dist = -1.0;
    let mut best = None;

    for c in samples.iter().step_by(6) {
        let dist = base.delta_e_oklab(c);
        if dist > max_dist && dist < 80.0 {
            max_dist = dist;
            best = Some(*c);
        }
    }

    best
}

fn find_most_saturated(samples: &[ColorRgb]) -> Option<ColorRgb> {
    samples
        .iter()
        .max_by(|a, b| {
            saturation(a)
                .partial_cmp(&saturation(b))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_png_bytes(w: u32, h: u32, fill: [u8; 3]) -> Vec<u8> {
        let img = image::RgbImage::from_fn(w, h, |_, _| image::Rgb(fill));
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        buf
    }

    fn write_tmp_png(fill: [u8; 3]) -> tempfile::NamedTempFile {
        let mut f = tempfile::Builder::new().suffix(".png").tempfile().unwrap();
        f.write_all(&make_png_bytes(10, 10, fill)).unwrap();
        f
    }

    // --- ColorRgb ---

    #[test]
    fn color_rgb_new_stores_fields() {
        let c = ColorRgb::new(10, 20, 30);
        assert_eq!((c.r, c.g, c.b), (10, 20, 30));
    }

    #[test]
    fn color_rgb_hex_format() {
        assert_eq!(ColorRgb::new(0, 0, 0).hex(), "#000000");
        assert_eq!(ColorRgb::new(255, 255, 255).hex(), "#ffffff");
        assert_eq!(ColorRgb::new(255, 0, 128).hex(), "#ff0080");
        assert_eq!(ColorRgb::new(16, 32, 48).hex(), "#102030");
    }

    #[test]
    fn delta_e_identical_is_zero() {
        let c = ColorRgb::new(100, 150, 200);
        assert!(c.delta_e_oklab(&c) < 0.01);
    }

    #[test]
    fn delta_e_black_white_is_large() {
        let black = ColorRgb::new(0, 0, 0);
        let white = ColorRgb::new(255, 255, 255);
        assert!(black.delta_e_oklab(&white) > 50.0);
    }

    #[test]
    fn delta_e_is_symmetric() {
        let a = ColorRgb::new(200, 100, 50);
        let b = ColorRgb::new(50, 100, 200);
        let diff = (a.delta_e_oklab(&b) - b.delta_e_oklab(&a)).abs();
        assert!(diff < 0.001);
    }

    #[test]
    fn delta_e_red_blue_differs_from_red_red() {
        let red = ColorRgb::new(255, 0, 0);
        let blue = ColorRgb::new(0, 0, 255);
        assert!(red.delta_e_oklab(&blue) > red.delta_e_oklab(&red));
    }

    #[test]
    fn color_rgb_eq() {
        assert_eq!(ColorRgb::new(1, 2, 3), ColorRgb::new(1, 2, 3));
        assert_ne!(ColorRgb::new(1, 2, 3), ColorRgb::new(1, 2, 4));
    }

    #[test]
    fn color_rgb_copy() {
        let a = ColorRgb::new(1, 2, 3);
        let b = a;
        assert_eq!(a, b);
    }

    // --- PaletteProfile ---

    fn make_palette(r: u8, g: u8, b: u8) -> PaletteProfile {
        PaletteProfile {
            primary: ColorRgb::new(r, g, b),
            secondary: ColorRgb::new(r / 2, g / 2, b / 2),
            surface: ColorRgb::new(18, 18, 20),
            accent: ColorRgb::new(255 - r, 255 - g, 255 - b),
        }
    }

    #[test]
    fn palette_distance_to_self_is_zero() {
        let p = make_palette(100, 150, 200);
        assert!(p.distance_to(&p) < 0.01);
    }

    #[test]
    fn palette_distance_black_white_is_large() {
        let black = PaletteProfile {
            primary: ColorRgb::new(0, 0, 0),
            secondary: ColorRgb::new(0, 0, 0),
            surface: ColorRgb::new(0, 0, 0),
            accent: ColorRgb::new(0, 0, 0),
        };
        let white = PaletteProfile {
            primary: ColorRgb::new(255, 255, 255),
            secondary: ColorRgb::new(255, 255, 255),
            surface: ColorRgb::new(255, 255, 255),
            accent: ColorRgb::new(255, 255, 255),
        };
        assert!(black.distance_to(&white) > 30.0);
    }

    #[test]
    fn palette_distance_surface_not_counted() {
        // surface is excluded from distance formula (only primary*0.5, secondary*0.3, accent*0.2)
        let mut a = make_palette(100, 100, 100);
        let mut b = make_palette(100, 100, 100);
        a.surface = ColorRgb::new(0, 0, 0);
        b.surface = ColorRgb::new(255, 255, 255);
        // Should still be 0 since surface isn't weighted
        assert!(a.distance_to(&b) < 0.01);
    }

    // --- extract_palette_from_image ---

    #[test]
    fn extract_palette_invalid_path_returns_none() {
        assert!(extract_palette_from_image(Path::new("/no/such/file.png")).is_none());
    }

    #[test]
    fn extract_palette_solid_red() {
        let f = write_tmp_png([220, 30, 30]);
        let p = extract_palette_from_image(f.path()).unwrap();
        // Primary should be reddish
        assert!(p.primary.r > p.primary.b, "primary should be red-dominant");
    }

    #[test]
    fn extract_palette_solid_blue() {
        let f = write_tmp_png([30, 30, 220]);
        let p = extract_palette_from_image(f.path()).unwrap();
        // Primary/surface should contain blue
        assert!(p.surface.b > p.surface.r || p.primary.b > p.primary.r);
    }

    #[test]
    fn extract_palette_solid_white_returns_some() {
        let f = write_tmp_png([255, 255, 255]);
        // White passes the lum > 235 guard, so vibrant_dominant falls back to first pixel
        let result = extract_palette_from_image(f.path());
        assert!(result.is_some());
    }

    #[test]
    fn extract_palette_solid_black_returns_some() {
        let f = write_tmp_png([0, 0, 0]);
        let result = extract_palette_from_image(f.path());
        assert!(result.is_some());
    }

    // --- internal helpers (via extract_palette_from_image) ---

    #[test]
    fn luminance_and_saturation_via_palette() {
        // Bright saturated green → high saturation, mid luminance
        let f = write_tmp_png([0, 200, 0]);
        let p = extract_palette_from_image(f.path()).unwrap();
        // Green channel should dominate
        assert!(p.primary.g > p.primary.r || p.accent.g > p.accent.r);
    }
}

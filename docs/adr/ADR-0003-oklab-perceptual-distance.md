# ADR-0003: Selection of Oklab for Perceptual Color Matching

| Metadata | Specification |
| :--- | :--- |
| **Status** | Accepted |
| **Date** | 2026-09-15 |
| **Author** | Jose Eduardo Rojas Jimenez |
| **Context** | Algorithmic matching between music album art dominant colors and wallpaper visual palettes. |

---

## 1. Context & Problem Statement

To determine whether a wallpaper visually harmonizes with an album cover, the engine requires a mathematical metric of chromatic distance.
* **sRGB:** Mathematically primitive, ignores non-linear human retinal cone sensitivity.
* **CIELAB (1976):** Industry standard, but known for severe blue-purple hue-shift distortion and non-uniformity in saturated gamuts.
* **CIEDE2000:** Computationally expensive ($> 15$ transcendental function calls per pixel pair).
* **Oklab (2020):** Formulated from perceptual lightness/chroma experiments; achieves superior hue linearity while using simple matrix multiplication and cube roots.

---

## 2. Decision

We adopted **Oklab** via the `palette` crate, computing Euclidean distance $\Delta E_{\text{ok}}$ across multi-cluster palette vectors (`primary`, `secondary`, `accent`).

```rust
pub fn delta_e_oklab(&self, other: &ColorRgb) -> f32 {
    let ok1 = self.to_oklab();
    let ok2 = other.to_oklab();
    ok1.distance(ok2) * 100.0
}
```

---

## 3. Consequences

* **Positive:** Matched wallpapers look aesthetically and perceptually unified with album covers; eliminates dissonant color clashes; executes in $< 50\mu\text{s}$ per comparison.
* **Negative:** Requires sRGB gamma linearization and LMS cone response transformations.

# ADR-0005: Compositor Abstraction & Noctalia Material You Pipeline

| Metadata | Specification |
| :--- | :--- |
| **Status** | Accepted |
| **Date** | 2026-09-15 |
| **Author** | Jose Eduardo Rojas Jimenez |
| **Context** | Projecting wallpapers onto Wayland surfaces and triggering dynamic compositor/shell theming. |

---

## 1. Context & Problem Statement

Wayland lacks a standardized universal protocol for setting desktop backgrounds across compositors. Hyprland, Sway, and Niri handle layer-shell surfaces differently. Furthermore, CachyOS Hyprland pairs Hyprland with Noctalia for Material Design 3 (M3) palette generation across the bar, terminal, and window borders.

---

## 2. Decision

We created the `CompositorBackend` trait:

```rust
pub trait CompositorBackend: Send + Sync {
    fn name(&self) -> &'static str;
    fn apply_wallpaper(&self, path: &Path, is_video: bool, palette: Option<&PaletteProfile>) -> Result<()>;
    fn pause(&self) -> Result<()>;
    fn resume(&self) -> Result<()>;
}
```

### Supported Implementations:
1. **`HyprlandNoctaliaBackend` (Primary):**
   * Manages hardware-accelerated video surfaces via `hypr-livewallpaper.service` (`mpvpaper -l bottom`).
   * Triggers Noctalia M3 extraction: `noctalia msg wallpaper-set "$POSTER"`.
   * Triggers Hyprland window border repainting: `hyprctl reload`.
2. **`CustomCommandBackend` (Generic):**
   * Executes user-specified command templates (e.g. `swww img '{file}'`, `waypaper --wallpaper '{file}'`), guaranteeing portability to Sway, Niri, River, or X11.
   * **Security Boundary:** Dispatches commands via structured argument vector parsing (`execvp` / `std::process::Command::args`) rather than invoking raw shell strings (`sh -c`), preventing command injection if filenames contain quotes or shell metacharacters.

---

## 3. Consequences

* **Positive:** Flawless integration with Noctalia and Hyprland without hardcoding VibeVeil exclusively to Hyprland; full portability across Wayland; shell injection vectors mitigated.
* **Negative:** Requires running Noctalia daemon or custom shell command on non-Hyprland environments.

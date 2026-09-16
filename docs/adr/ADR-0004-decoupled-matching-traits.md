# ADR-0004: Decoupled Strategy Pattern ("One Level Removed")

| Metadata | Specification |
| :--- | :--- |
| **Status** | Accepted |
| **Date** | 2026-09-15 |
| **Author** | Jose Eduardo Rojas Jimenez |
| **Context** | Abstracting wallpaper and media sources so the matching intelligence is completely independent of specific file vaults or directories. |

---

## 1. Context & Problem Statement

Initial implementations of wallpaper switchers rely on hardcoded paths (e.g. specific Pokémon lists). System requirements specify that the smart matching feature must be **"one level removed"**—an extensible engine capable of operating on *any* arbitrary media collection without code modifications.

---

## 2. Decision

We implemented the **Strategy Pattern** via the `MatchStrategy` trait:

```rust
pub trait MatchStrategy: Send + Sync {
    fn name(&self) -> &'static str;
    fn find_match(&self, track: &TrackContext, pool: &MediaPool) -> Option<MatchResult>;
}
```

### Concrete Implementations:
1. **`RulebookMatcher`:** Regex evaluations over metadata fields mapped to user-defined tags.
2. **`ColorDistanceMatcher`:** Mathematical minimization of Oklab $\Delta E$ across `MediaPool` color vectors.
3. **`ProceduralCanvasMatcher`:** Generative ambient blur + vinyl sleeve composition.
4. **`HybridMatcher`:** Orchestrates the cascading fallback pipeline.

---

## 3. Consequences

* **Positive:** Complete decoupling. The user can swap their entire wallpaper folder from Pokémon to Cyberpunk, Nature, or Architectural renders with zero recompilation.
* **Negative:** Requires an upfront indexation step to extract color vectors from new media files (cached in `pool_index.json`).

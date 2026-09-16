# ADR-0002: Native Async D-Bus Integration via `zbus`

| Metadata | Specification |
| :--- | :--- |
| **Status** | Accepted |
| **Date** | 2026-09-15 |
| **Author** | Jose Eduardo Rojas Jimenez |
| **Context** | Ingestion of real-time playback state and metadata from Spotify over the Linux user session bus. |

---

## 1. Context & Problem Statement

Many media integrations on Linux rely on either:
1. Spawning CLI helper subprocesses (`playerctl metadata --follow` via pipe).
2. Polling `playerctl` in a `while sleep 1` bash loop.
3. Using old C bindings (`libdbus`) which lack safe asynchronous thread integration.

Subprocess piping creates unwanted context switches and fragile subshell pipes, while continuous polling drains laptop batteries.

---

## 2. Decision

We integrated the native pure-Rust **`zbus` (v5.19)** crate.

### Architectural Advantages:
* **Zero-Process Overhead:** Communicates directly with `/run/user/1000/bus` through native Unix domain sockets without invoking external shell binaries.
* **Non-Blocking Async Event Loop:** Integrates with Tokio's `AsyncFd` epoll reactor; sleeps at 0% CPU until the kernel wakes the process on socket activity.
* **Broad Player Compatibility:** Auto-discovers any service beginning with `org.mpris.MediaPlayer2.*` (Spotify, VLC, MPD, Firefox, Cider, etc.), prioritizing Spotify when present.

---

## 3. Consequences

* **Positive:** Eliminates subprocess forking overhead; guarantees $< 5\text{ms}$ signal ingestion latency.
* **Negative:** Slightly increased binary size due to D-Bus type marshaling (`zvariant`).

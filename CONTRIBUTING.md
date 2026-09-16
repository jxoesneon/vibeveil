# Contributing to VibeVeil

Thank you for your interest in contributing to **VibeVeil**! We welcome bug reports, feature suggestions, architectural improvements, and code contributions.

---

## 1. Development Principles

VibeVeil is governed by strict architectural principles:
* **Zero-Polling Reactivity:** Always hook into asynchronous D-Bus signal streams (`zbus`), never use polling sleep loops in daemon runtime.
* **Perceptual Color Science:** Use the Oklab color space for delta-E calculations.
* **Strict Memory and CPU Budgets:** Steady-state background daemon memory must remain $<25\text{MB}$ RSS, and idle CPU must be $0.0\%$.
* **Safe Rust:** Enforce safe Rust; avoid `unsafe` blocks unless directly interacting with mandatory C/FFI boundaries.
* **Compositor Agnosticism:** Keep compositor dispatch modular through the `CompositorBackend` trait.

---

## 2. Setting Up Your Development Environment

### Prerequisites
* Rust toolchain (1.75 or later, 2024 edition compatible)
* `pkg-config` and `libdbus-1-dev` (Linux)
* Spotify or any MPRIS-compliant media player

### Building and Testing
```bash
# Clone the repository
git clone https://github.com/jxoesneon/vibeveil.git
cd vibeveil

# Check compilation and clippy lints
cargo check
cargo clippy --all-targets --all-features

# Run the test suite
cargo test

# Build release binary
cargo build --release
```

---

## 3. Submitting Pull Requests

1. Fork the repository and create your feature branch: `git checkout -b feat/my-new-feature`.
2. Ensure all tests pass: `cargo test`.
3. Verify formatting: `cargo fmt --all -- --check`.
4. Commit your changes using Conventional Commits (e.g. `feat: ...`, `fix: ...`, `docs: ...`).
5. Open a pull request against `main`.

---

## 4. Code of Conduct

Be welcoming, constructive, and respectful to all maintainers and contributors.

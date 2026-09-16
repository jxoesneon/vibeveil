# Packaging & Installation Guide

This document outlines the distribution and installation methods for **VibeVeil**.

---

## 1. Arch User Repository (AUR)

VibeVeil is available on the AUR as `vibeveil`.

### Installing with an AUR Helper

Using `paru`:
```bash
paru -S vibeveil
```

Using `yay`:
```bash
yay -S vibeveil
```

### Manual Build from PKGBUILD

```bash
git clone https://aur.archlinux.org/vibeveil.git
cd vibeveil
makepkg -si
```

Once installed, enable and start the user systemd unit:
```bash
systemctl --user enable --now vibeveil.service
```

---

## 2. Cargo (crates.io)

You can compile and install directly from crates.io:

```bash
cargo install vibeveil
```

Ensure prerequisites (`libdbus-1-dev`, `pkg-config`, `ffmpeg`) are installed on your system.

---

## 3. Precompiled Binary Tarball (GitHub Releases)

Precompiled, stripped 64-bit Linux binaries (`x86_64-unknown-linux-gnu`) are published with SHA-256 checksums on each release:

1. Download the latest `vibeveil-v*-x86_64-unknown-linux-gnu.tar.gz` from [Releases](https://github.com/jxoesneon/vibeveil/releases).
2. Extract the archive:
   ```bash
   tar -xzvf vibeveil-v*-x86_64-unknown-linux-gnu.tar.gz
   cd vibeveil-*-x86_64-unknown-linux-gnu
   ```
3. Install the binary and systemd service:
   ```bash
   install -m 755 vibeveil ~/.local/bin/vibeveil
   install -Dm644 vibeveil.service ~/.config/systemd/user/vibeveil.service
   systemctl --user daemon-reload
   systemctl --user enable --now vibeveil.service
   ```
4. Install shell completions (optional):
   ```bash
   # Bash
   install -Dm644 completions/vibeveil.bash ~/.local/share/bash-completion/completions/vibeveil
   # Zsh
   install -Dm644 completions/_vibeveil ~/.zsh/completion/_vibeveil
   # Fish
   install -Dm644 completions/vibeveil.fish ~/.config/fish/completions/vibeveil.fish
   ```

---

## 4. Building from Source

```bash
git clone https://github.com/jxoesneon/vibeveil.git
cd vibeveil
cargo build --release
install -m 755 target/release/vibeveil ~/.local/bin/vibeveil
```

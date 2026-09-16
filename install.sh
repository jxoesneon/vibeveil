#!/usr/bin/env bash
# ==============================================================================
# VibeVeil Universal Installer
# Music-Driven Dynamic Wallpaper & Semantic Desktop Theming Engine
# ==============================================================================
set -euo pipefail

BOLD='\033[1m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${CYAN}${BOLD}✦ VibeVeil Universal Installer${NC}"
echo -e "${CYAN}Universal music-driven dynamic wallpaper & semantic desktop theming engine${NC}\n"

# 1. Dependency checks
if ! command -v cargo >/dev/null 2>&1; then
    echo -e "${RED}Error: Rust/Cargo is not installed. Please install Rust from https://rustup.rs${NC}" >&2
    exit 1
fi

PREFIX="${PREFIX:-$HOME/.local}"
BIN_DIR="$PREFIX/bin"
SYSTEMD_USER_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/vibeveil"
BASH_COMP_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/bash-completion/completions"
ZSH_COMP_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/zsh/site-functions"
FISH_COMP_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/fish/vendor_completions.d"

# 2. Build Release Binary
echo -e "${BOLD}[1/5] Building VibeVeil (Release)...${NC}"
cargo build --release --locked

# 3. Install Binary
echo -e "${BOLD}[2/5] Installing binary to ${BIN_DIR}/vibeveil...${NC}"
mkdir -p "$BIN_DIR"
install -m 755 target/release/vibeveil "$BIN_DIR/vibeveil"

# 4. Generate and Install Shell Completions
echo -e "${BOLD}[3/5] Generating and installing shell completions...${NC}"
mkdir -p "$BASH_COMP_DIR" "$ZSH_COMP_DIR" "$FISH_COMP_DIR"
"$BIN_DIR/vibeveil" completions bash > "$BASH_COMP_DIR/vibeveil"
"$BIN_DIR/vibeveil" completions zsh > "$ZSH_COMP_DIR/_vibeveil"
"$BIN_DIR/vibeveil" completions fish > "$FISH_COMP_DIR/vibeveil.fish"

# 5. Install Systemd User Unit
echo -e "${BOLD}[4/5] Installing systemd user service...${NC}"
mkdir -p "$SYSTEMD_USER_DIR"
install -m 644 packaging/systemd/vibeveil.service "$SYSTEMD_USER_DIR/vibeveil.service"

if command -v systemctl >/dev/null 2>&1; then
    systemctl --user daemon-reload || true
fi

# 6. Initialize Default Configuration
echo -e "${BOLD}[5/5] Checking configuration directory...${NC}"
mkdir -p "$CONFIG_DIR"
"$BIN_DIR/vibeveil" init-config

echo -e "\n${GREEN}${BOLD}✔ VibeVeil successfully installed!${NC}\n"
echo -e "To start the background daemon immediately:"
echo -e "  ${CYAN}systemctl --user enable --now vibeveil.service${NC}\n"
echo -e "To check live status:"
echo -e "  ${CYAN}vibeveil status${NC}\n"
echo -e "To match currently playing song once:"
echo -e "  ${CYAN}vibeveil match --apply${NC}\n"

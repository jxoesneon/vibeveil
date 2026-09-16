#!/usr/bin/env bash
set -euo pipefail

KEY="${HOME}/.ssh/aur_ed25519"
if [ ! -f "$KEY" ]; then
    echo "Error: AUR SSH key not found at $KEY"
    exit 1
fi

AUR_DIR=$(mktemp -d)
trap 'rm -rf "$AUR_DIR"' EXIT

export GIT_SSH_COMMAND="ssh -i $KEY -o StrictHostKeyChecking=accept-new"

echo "Connecting to AUR (aur.archlinux.org)..."
if git clone ssh://aur@aur.archlinux.org/vibeveil.git "$AUR_DIR" 2>/dev/null; then
    echo "Cloned existing AUR repo."
else
    echo "Initializing package repository for vibeveil..."
    cd "$AUR_DIR"
    git init -b master
    git remote add origin ssh://aur@aur.archlinux.org/vibeveil.git
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cp "$SCRIPT_DIR/PKGBUILD" "$AUR_DIR/"
cp "$SCRIPT_DIR/.SRCINFO" "$AUR_DIR/"

cd "$AUR_DIR"
git add PKGBUILD .SRCINFO
if git diff --staged --quiet; then
    echo "No changes to commit for AUR."
else
    git commit -m "Release vibeveil v0.2.0"
    git push -u origin master
    echo "Successfully published vibeveil to AUR!"
fi

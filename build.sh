#!/bin/bash

# Karere build script
# Usage: ./build.sh [--dev]

set -e

cd "$(dirname "$0")"

BUILD_TYPE="prod"

while [[ $# -gt 0 ]]; do
    case $1 in
        --dev)
            BUILD_TYPE="dev"
            shift
            ;;
        --help)
            echo "Usage: $0 [--dev]"
            echo "  --dev      Build development version (uses Devel manifest)"
            echo "Default: Build production version"
            echo ""
            echo "The Flatpak will always be installed after building."
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

if [ "$BUILD_TYPE" = "dev" ]; then
    MANIFEST="packaging/io.github.tobagin.karere.Devel.yml"
    APP_ID="io.github.tobagin.karere.Dev"
    echo "Building development version..."
else
    MANIFEST="packaging/io.github.tobagin.karere.yml"
    APP_ID="io.github.tobagin.karere"
    echo "Building production version..."
fi

BUILD_DIR="build"

echo "Using manifest: $MANIFEST"
echo "Build directory: $BUILD_DIR"

# Shared local Flatpak repo (reused across all local apps)
REPO_DIR="$HOME/repo"
REMOTE_NAME="local"

echo "Running flatpak-builder..."
flatpak-builder --force-clean --disable-rofiles-fuse --install-deps-from=flathub --repo="$REPO_DIR" "$BUILD_DIR" "$MANIFEST"

echo "Installing from local repo..."
flatpak remote-add --user --no-gpg-verify --if-not-exists "$REMOTE_NAME" "$REPO_DIR"
# Uninstall any existing installation (may reference a stale remote)
flatpak uninstall --user -y "$APP_ID" 2>/dev/null || true
flatpak install --user -y "$REMOTE_NAME" "$APP_ID"

echo "Build and installation complete!"
echo "Run with: flatpak run $APP_ID"

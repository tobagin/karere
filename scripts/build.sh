#!/bin/bash

# Karere build script
# Usage: ./build.sh [--dev]

set -e

# Default to production build
BUILD_TYPE="prod"

# Parse arguments
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

# Set manifest based on build type
if [ "$BUILD_TYPE" = "dev" ]; then
    MANIFEST="packaging/io.github.tobagin.karere.Devel.yml"
    APP_ID="io.github.tobagin.karere.Devel"
    echo "Building development version..."
else
    MANIFEST="packaging/io.github.tobagin.karere.yml"
    APP_ID="io.github.tobagin.karere"
    echo "Building production version..."
fi

# Build directory (always 'build')
BUILD_DIR="build"

echo "Using manifest: $MANIFEST"
echo "Build directory: $BUILD_DIR"

# Build and install with Flatpak (always install)
echo "Running flatpak-builder (build and install)..."
flatpak-builder --force-clean --user --install --install-deps-from=flathub "$BUILD_DIR" "$MANIFEST"

echo "Build and installation complete!"
echo "Run with: flatpak run $APP_ID"
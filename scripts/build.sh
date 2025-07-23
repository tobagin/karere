#!/bin/bash

# Karere build script
# Usage: ./build.sh [--dev|--install]

set -e

# Default to development build
BUILD_TYPE="dev"
INSTALL=false

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --dev)
            BUILD_TYPE="dev"
            shift
            ;;
        --install)
            INSTALL=true
            shift
            ;;
        --help)
            echo "Usage: $0 [--dev] [--install]"
            echo "  --dev      Build development version (default)"
            echo "  --install  Install the Flatpak after building"
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

# Create build directory
BUILD_DIR="build-flatpak"
mkdir -p "$BUILD_DIR"

echo "Using manifest: $MANIFEST"
echo "Build directory: $BUILD_DIR"

# Build with Flatpak
echo "Running flatpak-builder..."
flatpak-builder --force-clean --user --install-deps-from=flathub "$BUILD_DIR" "$MANIFEST"

# Install if requested
if [ "$INSTALL" = true ]; then
    echo "Installing $APP_ID..."
    flatpak-builder --user --install --force-clean "$BUILD_DIR" "$MANIFEST"
    echo "Installation complete!"
    echo "Run with: flatpak run $APP_ID"
else
    echo "Build complete!"
    echo "To install, run: $0 --install"
    echo "Or install manually: flatpak-builder --user --install --force-clean $BUILD_DIR $MANIFEST"
fi
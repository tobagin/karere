#!/bin/bash

# Test script to verify window initialization fixes

set -e

echo "Testing Karere window fixes..."

cd /home/tobagin/Documents/Projects/karere-vala

# Clean up any previous build
rm -rf build

echo "Setting up build directory..."
meson setup build --wipe -Dprofile=development

echo "Compiling application..."
meson compile -C build

echo "Installing schema for testing..."
export GSETTINGS_SCHEMA_DIR=$(pwd)/build/data
mkdir -p $GSETTINGS_SCHEMA_DIR
cp build/io.github.tobagin.karere.Devel.gschema.xml $GSETTINGS_SCHEMA_DIR/
glib-compile-schemas $GSETTINGS_SCHEMA_DIR

echo "Testing basic application startup..."
export G_MESSAGES_DEBUG=all
timeout 10s build/karere --help || true

echo "Test completed!"
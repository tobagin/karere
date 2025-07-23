#!/usr/bin/env python3

"""
Test suite for build configuration validation.
Tests Meson build configuration, dependencies, and Flatpak manifest.
"""

import os
import sys
import subprocess
import json
import yaml
from pathlib import Path

def test_meson_file_exists():
    """Test that meson.build exists in project root."""
    meson_file = Path("meson.build")
    assert meson_file.exists(), "meson.build file must exist"
    print("✓ meson.build exists")

def test_meson_dependencies():
    """Test that meson.build contains required dependencies."""
    with open("meson.build", "r") as f:
        content = f.read()
    
    required_deps = [
        "gtk4",
        "libadwaita-1",
        "webkitgtk-6.0",
        "libsoup-3.0"
    ]
    
    for dep in required_deps:
        assert dep in content, f"Required dependency {dep} not found in meson.build"
        print(f"✓ {dep} dependency found")

def test_flatpak_manifest_exists():
    """Test that Flatpak manifests exist."""
    prod_manifest = Path("packaging/io.github.tobagin.karere.yml")
    dev_manifest = Path("packaging/io.github.tobagin.karere.Devel.yml")
    
    assert prod_manifest.exists(), "Production Flatpak manifest must exist"
    assert dev_manifest.exists(), "Development Flatpak manifest must exist"
    print("✓ Flatpak manifests exist")

def test_flatpak_permissions():
    """Test that Flatpak manifest has required permissions."""
    with open("packaging/io.github.tobagin.karere.yml", "r") as f:
        manifest = yaml.safe_load(f)
    
    required_permissions = [
        "--share=network",
        "--share=ipc",
        "--socket=fallback-x11",
        "--socket=wayland"
    ]
    
    finish_args = manifest.get("finish-args", [])
    for perm in required_permissions:
        assert perm in finish_args, f"Required permission {perm} not found"
        print(f"✓ {perm} permission found")

def test_blueprint_integration():
    """Test that Blueprint compiler is properly configured."""
    with open("meson.build", "r") as f:
        content = f.read()
    
    assert "blueprint" in content, "Blueprint integration not found in meson.build"
    assert "compile_blueprints" in content or "blueprints" in content, "Blueprint compilation not configured"
    print("✓ Blueprint integration configured")

def test_project_structure():
    """Test that required directories exist."""
    required_dirs = [
        "src",
        "data/ui", 
        "data/icons",
        "packaging",
        "tests",
        "po"
    ]
    
    for dir_path in required_dirs:
        assert Path(dir_path).exists(), f"Required directory {dir_path} does not exist"
        print(f"✓ {dir_path} directory exists")

def run_tests():
    """Run all build configuration tests."""
    print("Running build configuration tests...")
    
    test_functions = [
        test_project_structure,
        test_meson_file_exists,
        test_meson_dependencies,
        test_flatpak_manifest_exists,
        test_flatpak_permissions,
        test_blueprint_integration
    ]
    
    passed = 0
    failed = 0
    
    for test_func in test_functions:
        try:
            test_func()
            passed += 1
        except Exception as e:
            print(f"✗ {test_func.__name__}: {e}")
            failed += 1
    
    print(f"\nTest Results: {passed} passed, {failed} failed")
    return failed == 0

if __name__ == "__main__":
    success = run_tests()
    sys.exit(0 if success else 1)
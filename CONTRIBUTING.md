# Contributing to Karere

Thank you for your interest in contributing to Karere! This document provides guidelines and instructions for contributing to the project.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Making Changes](#making-changes)
- [Coding Standards](#coding-standards)
- [Testing](#testing)
- [Submitting Changes](#submitting-changes)
- [Reporting Issues](#reporting-issues)
- [Feature Requests](#feature-requests)

## Code of Conduct

This project follows the principles of respect, inclusivity, and professionalism. Please be considerate and constructive in all interactions.

## Getting Started

1. **Fork the repository** on GitHub
2. **Clone your fork** locally:
   ```bash
   git clone https://github.com/YOUR_USERNAME/karere.git
   cd karere
   ```
3. **Add the upstream remote**:
   ```bash
   git remote add upstream https://github.com/tobagin/karere.git
   ```

## Development Setup

### Prerequisites

- **SDKs**: `org.gnome.Sdk//49`, `org.freedesktop.Sdk.Extension.rust-stable//24.08`
- **Tools**: `flatpak-builder`, `cargo` (optional, for local checks)

### Building with Flatpak (Recommended)

```bash
# Install dependencies
flatpak install org.gnome.Sdk//49 org.gnome.Platform//49 org.freedesktop.Sdk.Extension.rust-stable//24.08

# Build and run
flatpak-builder --user --install --force-clean build packaging/io.github.tobagin.karere.Devel.yml
flatpak run io.github.tobagin.karere.Devel
```

### Local Development

For quick iteration (requires `cargo`, `gtk4-devel`, `libadwaita-devel` on host):

```bash
# Run via Cargo
cargo run

# Check/Lint
cargo check
cargo clippy
```

---

## Coding Standards

### Rust Code Style

- Follow standard Rust formatting (`cargo fmt`)
- Run `cargo clippy` to catch common issues
- Structs and Enums: `PascalCase`
- Functions and Variables: `snake_case`

### Blueprint UI Files

- Use `kebab-case` for filenames (e.g., `keyboard-shortcuts.blp`)
- Keep UI files in `data/ui/`

---

## Architecture Overview

Karere is built with:

- **Language**: Rust
- **Toolkit**: GTK4 + LibAdwaita
- **Web Engine**: WebKitGTK 6.0
- **UI**: Blueprint (`.blp`) compiled to GtkBuilder XML
- **Distribution**: Flatpak

### Project Structure

```
karere/
├── data/               # Application resources
│   ├── ui/             # Blueprint UI files
│   ├── icons/          # Application icons
│   └── sounds/         # Notification audio
├── src/                # Rust source code
├── packaging/          # Flatpak manifests
├── tools/              # Utility scripts
└── build.rs            # Build configuration
```

## Getting Help

- **Discussions**: Use [GitHub Discussions](https://github.com/tobagin/karere/discussions) for questions
- **Issues**: Use [GitHub Issues](https://github.com/tobagin/karere/issues) for bugs and feature requests

## License

By contributing to Karere, you agree that your contributions will be licensed under the GPL-3.0-or-later license.

---

Thank you for contributing to Karere!

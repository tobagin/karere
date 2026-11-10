# Contributing to Karere

Thank you for your interest in contributing to Karere! This document provides guidelines and instructions for contributing to the project.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
  - [Prerequisites](#prerequisites)
  - [Building with Flatpak (Recommended)](#building-with-flatpak-recommended)
  - [Local Development](#local-development)
- [Coding Standards](#coding-standards)
- [Architecture Overview](#architecture-overview)
- [Translations](#translations)
- [Getting Help](#getting-help)
- [License](#license)

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

- **Runtime and SDK**: `org.gnome.Platform//51` and `org.gnome.Sdk//51` (the `runtime-version` in `packaging/io.github.tobagin.karere.yml`), plus the `org.freedesktop.Sdk.Extension.rust-stable` extension listed under `sdk-extensions` in the same manifest
- **Tools**: `flatpak` and `flatpak-builder` for the Flatpak build
- **For a local `cargo build` or `cargo run` only**: a Rust toolchain that supports edition 2024, GTK 4 and libadwaita development files, `blueprint-compiler` on `PATH` (`build.rs` fails without it), and a CEF distribution (see below)

### Building with Flatpak (Recommended)

```bash
# Install the runtime and SDK
flatpak install flathub org.gnome.Platform//51 org.gnome.Sdk//51

# Build the working tree and install it as io.github.tobagin.karere.Devel
./build.sh --dev
flatpak run io.github.tobagin.karere.Devel
```

`./build.sh --dev` builds your checkout with `packaging/io.github.tobagin.karere.Devel.yml` (`-Dprofile=development`) and installs it. Options:

- `--regen-sources`: regenerate `packaging/cargo-sources.json` from `Cargo.lock`. Run it after any `Cargo.toml` or `Cargo.lock` change, because the Flatpak build is offline.
- `--no-install`: build without installing.
- `FLATPAK_BUILDER_EXTRA_ARGS`: extra flatpak-builder arguments, for example `--disable-rofiles-fuse` on hosts where rofiles-fuse cannot mount.

A plain `./build.sh` builds the production manifest at a tagged release commit, not your checkout, so use `--dev` to test your changes. See `./build.sh --help`.

### Local Development

For quick iteration outside Flatpak, first fetch CEF into `cef-binaries/`, then run through the wrapper, which sets `CEF_PATH`, compiles the development GSettings schema, and calls `cargo run`:

```bash
./download-cef.sh   # stock CEF distribution into cef-binaries/current
./run-dev.sh        # sets CEF_PATH when unset, then runs cargo run
```

`download-cef.sh` fetches stock CEF, without the proprietary codecs and Karere's CEF patches that the shipped Flatpak uses. Video playback and idle CPU can therefore differ between a local run and the Flatpak build. A bare `cargo build` needs `CEF_PATH` set to the CEF directory; `run-dev.sh` does this for you.

```bash
# Repository-wide Rust quality gates
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo check --workspace --all-targets
```

---

## Coding Standards

### Rust Code Style

- Install rustfmt and Clippy for the toolchain you build with.
- Check all Rust targets with `cargo fmt --all -- --check`.
- Treat every all-target Clippy warning as an error with `cargo clippy --workspace --all-targets -- -D warnings`.
- Structs and Enums: `PascalCase`
- Functions and Variables: `snake_case`

### Blueprint UI Files

- Use `kebab-case` for filenames (e.g., `keyboard-shortcuts.blp`)
- Keep UI files in `data/ui/`

---

## Architecture Overview

Karere is built with:

- **Language**: Rust (edition 2024)
- **Toolkit**: GTK4 + libadwaita
- **Web engine**: Chromium Embedded Framework (CEF, Chromium 156). CEF renders WhatsApp Web off-screen and composites it into a GTK `GLArea`.
- **Injected scripts**: `data/js/` runs inside the WhatsApp Web page and reports to Rust through `window.karere.send`
- **UI**: Blueprint (`data/ui/*.blp`), compiled to GtkBuilder `.ui` files at build time
- **Distribution**: Flatpak on the GNOME 51 runtime

### Project Structure

```
karere/
├── data/               # Application resources
│   ├── ui/             # Blueprint UI files
│   ├── js/             # Scripts injected into WhatsApp Web
│   ├── js-deferred/    # Additional scripts embedded in the binary
│   ├── icons/          # Application icons
│   └── sounds/         # Notification audio
├── src/                # Rust source code
├── tests/              # Test scripts and fixtures
├── tools/              # Utility scripts
├── packaging/          # Flatpak manifests
├── build.rs            # Compiles Blueprint files and resources
├── build.sh            # Flatpak build wrapper
├── download-cef.sh     # Fetches the CEF distribution
└── run-dev.sh          # Local run wrapper (sets CEF_PATH)
```

## Translations

Translation catalogs live in `po/`. They were machine-translated and have not been reviewed by native speakers yet (see `po/README.md`). Corrections are welcome as pull requests. To regenerate the template and merge catalogs after changing translatable strings, run `tools/update-po.sh`. To add a new language, add its code to `po/LINGUAS`, copy `po/karere.pot` to `po/<code>.po`, translate it, and open a pull request. `tools/verify-po.sh` checks that the catalogs are valid.

## Getting Help

- **Discussions**: Use [GitHub Discussions](https://github.com/tobagin/karere/discussions) for questions
- **Issues**: Use [GitHub Issues](https://github.com/tobagin/karere/issues) for bugs and feature requests

## License

By contributing to Karere, you agree that your contributions will be licensed under the GPL-3.0-or-later license.

---

Thank you for contributing to Karere!

# Building Karere from Source

> Complete guide for developers and contributors

## Overview

This guide covers building Karere from source code, including development environment setup, build procedures, and troubleshooting. Whether you're contributing to the project or customizing Karere for your needs, this documentation will get you started.

## Development Environment Setup

### System Requirements

#### Minimum Requirements
- Linux distribution with modern development tools
- GTK4 4.14 or later development headers
- LibAdwaita 1.5 or later development headers
- WebKitGTK 6.0 development headers
- Vala compiler 0.56 or later
- Meson build system 0.59 or later

#### Recommended Environment
- Recent Linux distribution (Ubuntu 22.04+, Fedora 38+, Arch Linux current)
- 4GB+ RAM for comfortable compilation
- Fast SSD for development work
- Git for version control
- Modern code editor with Vala support

### Installing Dependencies

#### Ubuntu/Debian
```bash
# Update package lists
sudo apt update

# Core development tools
sudo apt install \
  build-essential \
  git \
  meson \
  ninja-build \
  valac \
  pkg-config \
  gettext \
  desktop-file-utils \
  appstream-util

# GTK and LibAdwaita development
sudo apt install \
  libgtk-4-dev \
  libadwaita-1-dev \
  libwebkitgtk-6.0-dev \
  libglib2.0-dev \
  libgio-2.0-dev

# Blueprint compiler (for UI files)
sudo apt install blueprint-compiler

# Additional development tools
sudo apt install \
  gdb \
  valgrind \
  lcov \
  gi-docgen

# Documentation tools
sudo apt install \
  pandoc \
  graphviz
```

#### Fedora
```bash
# Core development tools
sudo dnf install \
  gcc \
  git \
  meson \
  ninja-build \
  vala \
  pkgconfig \
  gettext \
  desktop-file-utils \
  libappstream-glib

# GTK and LibAdwaita development
sudo dnf install \
  gtk4-devel \
  libadwaita-devel \
  webkitgtk6.0-devel \
  glib2-devel

# Blueprint compiler
sudo dnf install blueprint-compiler

# Development tools
sudo dnf install \
  gdb \
  valgrind \
  lcov \
  gi-docgen
```

#### Arch Linux
```bash
# Core development tools
sudo pacman -S \
  base-devel \
  git \
  meson \
  ninja \
  vala \
  pkgconf \
  gettext \
  desktop-file-utils \
  appstream-glib

# GTK and LibAdwaita development
sudo pacman -S \
  gtk4 \
  libadwaita \
  webkitgtk-6.0 \
  glib2

# Blueprint compiler
sudo pacman -S blueprint-compiler

# Development tools
sudo pacman -S \
  gdb \
  valgrind \
  lcov
```

### Code Editor Setup

#### VS Code with Vala Extension
```bash
# Install VS Code
snap install code --classic

# Install Vala extension
code --install-extension prince781.vala
```

#### Vim/Neovim with Vala Support
```bash
# Install vala.vim syntax highlighting
mkdir -p ~/.vim/syntax
wget -O ~/.vim/syntax/vala.vim \
  https://raw.githubusercontent.com/arrufat/vala.vim/master/syntax/vala.vim

# Add to ~/.vimrc
echo 'au BufRead,BufNewFile *.vala setfiletype vala' >> ~/.vimrc
```

#### GNOME Builder (Recommended for GNOME development)
```bash
flatpak install flathub org.gnome.Builder
```

## Getting the Source Code

### Clone the Repository

```bash
# Clone main repository
git clone https://github.com/tobagin/karere-vala.git
cd karere-vala

# Check available branches
git branch -a

# Switch to development branch (if desired)
git checkout development
```

### Fork for Contributing
If you plan to contribute:

1. **Fork on GitHub**: Click "Fork" on the project page
2. **Clone your fork**:
   ```bash
   git clone https://github.com/YOUR_USERNAME/karere-vala.git
   cd karere-vala
   ```
3. **Add upstream remote**:
   ```bash
   git remote add upstream https://github.com/tobagin/karere-vala.git
   ```

### Repository Structure
```
karere-vala/
├── src/                    # Vala source files
├── data/                   # Application data and resources
│   ├── ui/                 # Blueprint UI files
│   ├── icons/              # Application icons
│   └── resources.xml       # GResource definition
├── po/                     # Internationalization files
├── tests/                  # Unit tests
├── docs/                   # Documentation
├── scripts/                # Build and utility scripts
├── packaging/              # Flatpak manifests
├── meson.build            # Main build configuration
└── meson_options.txt      # Build options
```

## Build Methods

### Method 1: Flatpak Development Build (Recommended)

The easiest way to build and test Karere is using the included Flatpak development configuration.

#### Quick Development Build
```bash
# Build and install development version
./scripts/build.sh --dev --install

# Launch development version
flatpak run io.github.tobagin.karere.Devel
```

#### Manual Flatpak Build
```bash
# Install Flatpak Builder
sudo apt install flatpak-builder  # Ubuntu/Debian
sudo dnf install flatpak-builder  # Fedora
sudo pacman -S flatpak-builder    # Arch Linux

# Build with flatpak-builder
flatpak-builder --user --install --force-clean \
  build-dir packaging/io.github.tobagin.karere.Devel.yml

# Run the built application
flatpak run io.github.tobagin.karere.Devel
```

#### Flatpak Build Options
```bash
# Build without installing
flatpak-builder --force-clean \
  build-dir packaging/io.github.tobagin.karere.Devel.yml

# Build for specific architecture
flatpak-builder --arch=x86_64 --force-clean \
  build-dir packaging/io.github.tobagin.karere.Devel.yml

# Build with additional debugging
flatpak-builder --ccache --force-clean \
  build-dir packaging/io.github.tobagin.karere.Devel.yml
```

### Method 2: Native Meson Build

For development and debugging, you may prefer native builds.

#### Basic Native Build
```bash
# Configure build directory
meson setup build --prefix=/usr/local

# Compile the project
meson compile -C build

# Run tests
meson test -C build

# Install (optional)
sudo meson install -C build
```

#### Build Configuration Options
```bash
# Debug build with all debugging symbols
meson setup build --buildtype=debug --prefix=/usr/local

# Release build with optimizations
meson setup build --buildtype=release --prefix=/usr/local

# Custom build with specific options
meson setup build \
  --buildtype=debug \
  --prefix=/usr/local \
  -Dtests=true \
  -Ddocs=true \
  -Dprofile=development
```

#### Available Build Options
Check available options:
```bash
meson configure build
```

Common options:
- `-Dtests=true/false`: Enable/disable unit tests
- `-Ddocs=true/false`: Enable/disable documentation generation
- `-Dprofile=default/development/release`: Build profile
- `-Denable-tracing=true/false`: Enable debug tracing

### Method 3: Development Container

For consistent development environments:

#### Using Docker
```bash
# Create development container
docker build -f Dockerfile.dev -t karere-dev .

# Run development container
docker run -it --rm \
  -v $(pwd):/workspace \
  -v /tmp/.X11-unix:/tmp/.X11-unix \
  -e DISPLAY=$DISPLAY \
  karere-dev
```

#### Using Podman
```bash
# Build container
podman build -f Dockerfile.dev -t karere-dev .

# Run with X11 forwarding
podman run -it --rm \
  -v $(pwd):/workspace:Z \
  -v /tmp/.X11-unix:/tmp/.X11-unix:Z \
  -e DISPLAY=$DISPLAY \
  karere-dev
```

## Build Scripts

### Using the Build Script

The project includes a convenient build script:

```bash
# Show help
./scripts/build.sh --help

# Development build and install
./scripts/build.sh --dev --install

# Production build
./scripts/build.sh --prod

# Clean build (removes previous build artifacts)
./scripts/build.sh --clean --dev --install
```

#### Build Script Options
- `--dev`: Build development version with Devel app ID
- `--prod`: Build production version
- `--install`: Install after successful build
- `--clean`: Clean previous build artifacts
- `--help`: Show usage information

### Custom Build Scripts

Create custom build configurations:

```bash
#!/bin/bash
# custom-debug-build.sh

# Set up debug environment
export CFLAGS="-g -O0 -DDEBUG"
export VALAFLAGS="--debug --verbose"

# Configure with debug options
meson setup build-debug \
  --buildtype=debug \
  --prefix=/usr/local \
  -Dtests=true \
  -Dprofile=development

# Build with verbose output
meson compile -C build-debug -v

# Run tests
meson test -C build-debug --verbose
```

## Development Workflow

### Setting Up Development Environment

#### Configure Git
```bash
# Set up git (if not already done)
git config --global user.name "Your Name"
git config --global user.email "your.email@example.com"

# Set up project-specific settings
cd karere-vala
git config user.name "Your Name"
git config user.email "your.email@example.com"
```

#### Pre-commit Hooks
```bash
# Install pre-commit hooks (if available)
pip install pre-commit
pre-commit install

# Or create custom pre-commit hook
cat > .git/hooks/pre-commit << 'EOF'
#!/bin/bash
# Run basic checks before commit
meson test -C build
if [ $? -ne 0 ]; then
    echo "Tests failed, commit aborted"
    exit 1
fi
EOF
chmod +x .git/hooks/pre-commit
```

### Development Build Cycle

#### 1. Make Changes
Edit source files in your preferred editor.

#### 2. Build and Test
```bash
# Quick build (only rebuild changed files)
meson compile -C build

# Run specific tests
meson test -C build test_name

# Run all tests
meson test -C build
```

#### 3. Test Changes
```bash
# Run from build directory (native build)
./build/src/karere

# Or reinstall Flatpak development version
./scripts/build.sh --dev --install
flatpak run io.github.tobagin.karere.Devel
```

#### 4. Debug Issues
```bash
# Run with debugger
gdb ./build/src/karere

# Run with Valgrind
valgrind --tool=memcheck ./build/src/karere

# Run with debug output
KARERE_DEBUG=1 ./build/src/karere
```

### Code Quality Tools

#### Static Analysis
```bash
# Run Vala static analysis
valac --version  # Ensure using recent Vala

# Check for common issues
find src/ -name "*.vala" -exec vala-lint {} \;
```

#### Memory Debugging
```bash
# Check for memory leaks
valgrind --leak-check=full --show-leak-kinds=all \
  ./build/src/karere

# Generate memory usage report
valgrind --tool=massif ./build/src/karere
ms_print massif.out.*
```

#### Performance Profiling
```bash
# Profile with perf
perf record ./build/src/karere
perf report

# Profile with callgrind
valgrind --tool=callgrind ./build/src/karere
kcachegrind callgrind.out.*
```

## Testing

### Running Tests

#### Unit Tests
```bash
# Run all tests
meson test -C build

# Run specific test
meson test -C build test_application

# Run tests with verbose output
meson test -C build --verbose

# Run tests with debugging
meson test -C build --gdb
```

#### Integration Tests
```bash
# Run Flatpak integration tests
./scripts/test-flatpak.sh

# Test different build configurations
for buildtype in debug release; do
    meson setup build-$buildtype --buildtype=$buildtype
    meson test -C build-$buildtype
done
```

### Writing Tests

#### Unit Test Example
```vala
// tests/test_logger.vala
using Karere;

void test_logger_basic() {
    var logger = new Logger();
    logger.log(LogLevel.INFO, "Test message");
    // Add assertions here
}

void main(string[] args) {
    Test.init(ref args);
    Test.add_func("/karere/logger/basic", test_logger_basic);
    Test.run();
}
```

#### Adding Tests to Build
```meson
# tests/meson.build
test_logger = executable(
    'test_logger',
    'test_logger.vala',
    dependencies: karere_deps,
    install: false
)

test('logger', test_logger)
```

## Debugging

### Debug Builds

#### Configure Debug Build
```bash
meson setup build-debug \
  --buildtype=debug \
  -Db_sanitize=address \
  -Db_lundef=false
```

#### Debug with GDB
```bash
# Compile with debug symbols
meson compile -C build-debug

# Run with GDB
gdb ./build-debug/src/karere

# GDB commands
(gdb) run
(gdb) bt          # backtrace on crash
(gdb) info locals # show local variables
(gdb) break main  # set breakpoint
```

#### Debug with Valgrind
```bash
# Memory debugging
valgrind --tool=memcheck \
  --leak-check=full \
  --track-origins=yes \
  ./build-debug/src/karere

# Thread debugging
valgrind --tool=helgrind ./build-debug/src/karere
```

### WebKit Debugging

#### Enable WebKit Inspector
```bash
# Set environment variable
export WEBKIT_INSPECTOR_SERVER=127.0.0.1:9999

# Run Karere
./build/src/karere

# Connect with browser to inspect WebKit content
# Open http://127.0.0.1:9999 in browser
```

#### WebKit Console Debugging
```vala
// Enable WebKit debugging in source
webkit_settings_set_enable_developer_extras(settings, TRUE);
webkit_settings_set_enable_write_console_messages_to_stdout(settings, TRUE);
```

## Troubleshooting Build Issues

### Common Build Errors

#### Missing Dependencies
```bash
# Error: Package 'gtk4' not found
sudo apt install libgtk-4-dev  # Ubuntu/Debian
sudo dnf install gtk4-devel    # Fedora
sudo pacman -S gtk4           # Arch Linux
```

#### Vala Compilation Errors
```bash
# Error: vala compiler version too old
# Check Vala version
valac --version

# Update Vala (Ubuntu/Debian)
sudo add-apt-repository ppa:vala-team/next
sudo apt update
sudo apt install vala-dev
```

#### Blueprint Compilation Issues
```bash
# Error: blueprint-compiler not found
# Check if blueprint-compiler is installed
which blueprint-compiler

# Install if missing
sudo apt install blueprint-compiler  # Ubuntu 22.04+
# Or install from source for older distributions
```

#### Meson Configuration Issues
```bash
# Clear meson cache and reconfigure
rm -rf build/
meson setup build --wipe
```

### Platform-Specific Issues

#### Ubuntu 20.04 (GTK4 Support)
```bash
# Add PPA for newer GTK4
sudo add-apt-repository ppa:mhall119/ppa
sudo apt update
sudo apt install libgtk-4-dev libadwaita-1-dev
```

#### Arch Linux (Rolling Release Issues)
```bash
# Update system first
sudo pacman -Syu

# Check for AUR packages if needed
yay -S blueprint-compiler-git  # If not in official repos
```

#### Fedora (Package Naming)
```bash
# Note different package names
sudo dnf install \
  webkitgtk6.0-devel \  # Not webkitgtk-6.0-devel
  libadwaita-devel      # Not adwaita-devel
```

## Documentation Generation

### API Documentation
```bash
# Generate API docs with gi-docgen
meson configure build -Ddocs=true
meson compile -C build

# Docs will be in build/docs/
```

### User Documentation
```bash
# Convert markdown to HTML (if pandoc available)
pandoc docs/user-guide/getting-started.md -o getting-started.html

# Generate PDF documentation
pandoc docs/user-guide/*.md -o karere-user-guide.pdf
```

## Packaging

### Creating Source Archives
```bash
# Create source tarball
git archive --format=tar.gz --prefix=karere-vala-1.0/ HEAD > karere-vala-1.0.tar.gz

# Create source archive with submodules (if any)
git archive --format=tar.gz --prefix=karere-vala-1.0/ HEAD > karere-vala-1.0.tar.gz
```

### Distribution Packages

#### Debian Package
```bash
# Install packaging tools
sudo apt install devscripts debhelper

# Create Debian package (if debian/ directory exists)
debuild -us -uc
```

#### RPM Package
```bash
# Install RPM build tools
sudo dnf install rpm-build rpmdevtools

# Create RPM (if .spec file exists)
rpmbuild -ba karere.spec
```

## Contributing Guidelines

### Code Style
- Follow the [code style guide](../../.agent-os/standards/code-style.md)
- Use consistent indentation (4 spaces)
- Write meaningful commit messages
- Add tests for new functionality

### Submitting Changes
1. Create a feature branch: `git checkout -b feature-name`
2. Make your changes and test thoroughly
3. Commit with descriptive messages
4. Push to your fork: `git push origin feature-name`
5. Create a pull request on GitHub

### Testing Before Submission
```bash
# Run full test suite
meson test -C build

# Test Flatpak build
./scripts/build.sh --dev --install

# Test with different configurations
meson setup build-release --buildtype=release
meson test -C build-release
```

---

*For additional development resources, see the [Contributing Guide](contributing.md) and [Testing Guide](testing.md).*
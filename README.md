# Karere

A modern, native GTK4/LibAdwaita wrapper for WhatsApp Web that provides seamless integration with the Linux desktop environment.

![Karere Application](https://raw.githubusercontent.com/tobagin/karere-vala/main/data/screenshots/main-window.png)

## Features

- **Native Desktop Integration**: True native notifications using GNotification, not browser notifications
- **LibAdwaita Theming**: Full support for Light, Dark, and System themes with native styling
- **Comprehensive Logging**: Structured logging system with configurable levels and file/console output
- **Crash Reporting**: Optional crash detection and reporting system for improved stability
- **WebKitGTK Optimization**: Efficient resource usage compared to browser-based solutions
- **Spell Checking**: Multi-language spell checking support
- **Privacy Controls**: Granular settings for logging, notifications, and crash reporting

## Installation

### Flatpak (Recommended)

#### Development Version
```bash
# Clone the repository
git clone https://github.com/tobagin/karere-vala.git
cd karere-vala

# Build and install development version
./scripts/build.sh --dev --install
```

#### Production Version (Coming Soon)
Karere will be available on Flathub once released.

## Building from Source

### Prerequisites

- GTK4 4.14+
- LibAdwaita 1.5+
- WebKitGTK 6.0
- Vala compiler
- Meson build system
- Blueprint compiler 0.18+

### Build Steps

```bash
# Install dependencies (Ubuntu/Debian)
sudo apt install valac meson build-essential libgtk-4-dev libadwaita-1-dev libwebkitgtk-6.0-dev blueprint-compiler

# Clone and build
git clone https://github.com/tobagin/karere-vala.git
cd karere-vala

# Configure and build
meson setup build --prefix=/usr
meson compile -C build

# Install (optional)
sudo meson install -C build
```

## Usage

### Basic Usage

Launch Karere from your applications menu or run:
```bash
karere
```

The application will load WhatsApp Web and provide native desktop integration.

### Preferences

Access preferences through the application menu or keyboard shortcut to configure:

- **Appearance**: Theme selection (Light, Dark, System)
- **Privacy**: Logging controls and crash reporting settings
- **Notifications**: Native notification preferences

### Keyboard Shortcuts

- `Ctrl+,` - Open Preferences
- `Ctrl+Q` - Quit Application
- `F11` - Toggle Fullscreen

## Architecture

Karere is built using modern GNOME technologies:

- **Vala**: Primary programming language for type safety and productivity
- **GTK4**: Modern toolkit with excellent Wayland support
- **LibAdwaita**: Native GNOME design language and components
- **WebKitGTK 6.0**: Efficient web rendering engine
- **Blueprint**: Declarative UI definition language
- **Flatpak**: Secure application distribution

## Privacy & Security

Karere is designed with privacy in mind:

- **Sandboxed**: Runs in a Flatpak sandbox with minimal permissions
- **Opt-in Telemetry**: All logging and crash reporting is disabled by default
- **Local Storage**: Uses standard user data directories, no external services
- **Transparent**: Open source code available for audit

## Contributing

Contributions are welcome! Please see our development documentation:

- [Development Setup](docs/DEVELOPMENT.md) - Setting up your development environment
- [Code Style Guide](docs/CODE_STYLE.md) - Coding standards and conventions
- [Architecture Overview](docs/ARCHITECTURE.md) - Technical architecture details

### Development Workflow

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Run tests: `meson test -C build`
5. Submit a pull request

## Documentation

- [User Documentation](docs/) - User guides and help
- [Development Documentation](docs/DEVELOPMENT.md) - Developer resources
- [Flathub Compliance](docs/FLATHUB_COMPLIANCE.md) - Distribution requirements

## License

Karere is licensed under the GNU General Public License v3.0 or later. See [LICENSE](LICENSE) for the full license text.

## Support

- **Bug Reports**: [GitHub Issues](https://github.com/tobagin/karere-vala/issues)
- **Discussions**: [GitHub Discussions](https://github.com/tobagin/karere-vala/discussions)
- **Documentation**: [Project Wiki](https://github.com/tobagin/karere-vala/wiki)

## Acknowledgments

- **GNOME Project**: For the excellent GTK4 and LibAdwaita frameworks
- **WebKitGTK Team**: For the efficient web rendering engine
- **WhatsApp Inc.**: For WhatsApp Web
- **Vala Community**: For the productive programming language

## Screenshots

| Light Theme | Dark Theme |
|-------------|------------|
| ![Light](data/screenshots/main-window.png) | ![Dark](data/screenshots/dark-theme.png) |

| Preferences | Notifications |
|-------------|---------------|
| ![Preferences](data/screenshots/preferences-general.png) | ![Notifications](data/screenshots/preferences-privacy.png) |

---

**Karere** - Native WhatsApp Web client for Linux desktop environments.
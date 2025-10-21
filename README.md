# Karere

A modern, native GTK4/LibAdwaita wrapper for WhatsApp Web that provides seamless integration with the Linux desktop environment.

![Karere Application](https://raw.githubusercontent.com/tobagin/karere/main/data/screenshots/main-window.png)

## 🎉 Version 1.0.3 - Stable Release

**Karere 1.0.3** is the latest stable release with fully working notifications, enhanced accessibility, and comprehensive WhatsApp Web integration.

### ✨ Key Features

- **✅ Working Notifications**: WhatsApp notifications work perfectly with proper banner persistence
- **🖼️ Image Paste**: Ctrl+V works for both text and images in WhatsApp Web
- **📥 Download Manager**: Custom download directory with toast notifications
- **📝 Spell Checking**: 80+ language dictionaries with auto-detection
- **♿ Enhanced Accessibility**: Screen reader optimization, keyboard navigation, and focus indicators
- **⌨️ Keyboard Shortcuts**: Comprehensive shortcuts dialog with all available commands

### 🆕 What's New in 1.0.3

- Updated build configuration and dependencies
- Enhanced packaging for better compatibility
- Minor bug fixes and stability improvements

## Features

### Core Features
- **Native Desktop Integration**: True native notifications using GNotification with persistent permission state
- **WhatsApp Web Integration**: Full WhatsApp Web functionality with proper notification handling
- **LibAdwaita Theming**: Full support for Light, Dark, and System themes with native styling
- **WebKitGTK Optimization**: Efficient resource usage with persistent storage for cookies and sessions
- **Image & Text Paste**: Seamless Ctrl+V support for both images and text content
- **Download Manager**: Custom download directory selection with completion notifications

### Accessibility Support
- **Screen Reader Optimization**: Full ARIA labels and semantic HTML
- **Keyboard Navigation**: Complete keyboard-only navigation with visible focus indicators (82 focusable elements)
- **High Contrast Mode**: Automatic detection and adaptation
- **Reduced Motion**: Respects system reduce-motion preferences
- **Configurable Shortcuts**: All keyboard shortcuts can be enabled/disabled
- **Focus Management**: Logical focus chain for efficient navigation

### Spell Checking
- **Multi-Language Support**: 80+ dictionaries from LibreOffice
- **Auto-Detection**: Automatically detect language from system locale
- **Manual Selection**: Choose specific languages for spell checking
- **Real-time Checking**: Inline spell checking as you type

### Privacy & Customization
- **Granular Notification Controls**: Customize notification behavior and previews
- **Privacy Settings**: Control data logging and crash reporting
- **Theme Selection**: Choose between Light, Dark, or System theme
- **Zoom Control**: Configurable WebView zoom (disabled by default)

## Installation

### Flatpak (Recommended)

#### From Flathub
```bash
flatpak install flathub io.github.tobagin.karere
```

#### Development Version
```bash
# Clone the repository
git clone https://github.com/tobagin/karere.git
cd karere

# Build and install development version
./scripts/build.sh --dev
```

**Note**: After installation, you'll need to scan the QR code with your mobile WhatsApp to connect.

## Usage

### Basic Usage

Launch Karere from your applications menu or run:
```bash
flatpak run io.github.tobagin.karere
```

The application will load WhatsApp Web and provide native desktop integration.

### Preferences

Access preferences through the application menu or keyboard shortcut (`Ctrl+,`) to configure:

- **General**: Theme selection, developer tools
- **Accessibility**: Keyboard shortcuts, focus indicators, high contrast, reduced motion, zoom settings, screen reader optimization
- **Notifications**: Native notification preferences, preview settings, background notifications
- **Spell Checking**: Multi-language spell checking with auto-detect

### Keyboard Shortcuts

#### Standard
- `Ctrl+,` - Open Preferences
- `Ctrl+Q` - Quit Application
- `F1` - Show Keyboard Shortcuts Help

#### Zoom
- `Ctrl++` - Zoom In
- `Ctrl+-` - Zoom Out
- `Ctrl+0` - Reset Zoom

#### Developer (when enabled)
- `Ctrl+Shift+D` - Open Developer Tools
- `Ctrl+R` - Reload Page

#### WhatsApp Web
- `Ctrl+F` - Find in Chat
- `Ctrl+Shift+F` - Search Chats

### Accessibility Features

Karere includes comprehensive accessibility support:

- **Screen Reader Support**: Full ARIA labels and semantic HTML
- **Keyboard Navigation**: Complete keyboard-only navigation with visible focus indicators
- **High Contrast Mode**: Automatic detection and adaptation
- **Reduced Motion**: Respects system reduce-motion preferences
- **Configurable Shortcuts**: All keyboard shortcuts can be enabled/disabled
- **Focus Management**: 82 focusable elements in a logical focus chain

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

- **Bug Reports**: [GitHub Issues](https://github.com/tobagin/karere/issues)
- **Discussions**: [GitHub Discussions](https://github.com/tobagin/karere/discussions)
- **Documentation**: [Project Wiki](https://github.com/tobagin/karere/wiki)

## Acknowledgments

- **GNOME Project**: For the excellent GTK4 and LibAdwaita frameworks
- **WebKitGTK Team**: For the efficient web rendering engine
- **WhatsApp Inc.**: For WhatsApp Web
- **Vala Community**: For the productive programming language

## Screenshots

| Main Window | About Dialog |
|-------------|--------------|
| ![Main Window](data/screenshots/main-window.png) | ![About](data/screenshots/about.png) |

| Preferences | Shortcuts |
|-------------|-----------|
| ![Preferences](data/screenshots/preferences.png) | ![Shortcuts](data/screenshots/shortcuts.png) |

---

**Karere** - Native WhatsApp Web client for Linux desktop environments.

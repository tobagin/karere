# Getting Started with Karere

Welcome to Karere, a modern, native GTK4/LibAdwaita wrapper for WhatsApp Web that provides seamless integration with the Linux desktop environment.

## What is Karere?

Karere transforms the WhatsApp Web experience by providing:

- **Native Desktop Integration**: True desktop notifications using GNotification instead of browser notifications
- **LibAdwaita Theming**: Full support for Light, Dark, and System themes with native GNOME styling
- **Comprehensive Logging**: Structured logging system for debugging and troubleshooting
- **Crash Reporting**: Optional crash detection and reporting for improved stability
- **Optimized Performance**: Efficient resource usage with WebKitGTK 6.0
- **Privacy Controls**: Granular settings for notifications, logging, and data collection

## System Requirements

### Minimum Requirements
- Linux distribution with GTK4 support
- GTK4 4.14 or later
- LibAdwaita 1.5 or later
- WebKitGTK 6.0
- Display resolution: 768x576 or higher

### Recommended Requirements
- Modern Linux distribution (Ubuntu 22.04+, Fedora 38+, openSUSE Tumbleweed)
- GNOME 44+ or compatible desktop environment
- 4GB RAM or more
- Stable internet connection for WhatsApp Web

### Supported Desktop Environments
- **GNOME** (Primary target, best integration)
- **Pantheon** (elementary OS)
- **Budgie** (Good LibAdwaita support)
- **Cinnamon** (Basic functionality)
- **XFCE** (Limited theming integration)
- **KDE Plasma** (Basic functionality, no theme integration)

## Installation

### Method 1: Flatpak (Recommended)

Flatpak is the recommended installation method as it provides sandboxing, automatic updates, and consistent behavior across distributions.

#### Install from Flathub (Coming Soon)
Once Karere is available on Flathub:
```bash
flatpak install flathub io.github.tobagin.karere
```

#### Install Development Version
For the latest development version with newest features:

1. **Prerequisites**: Ensure you have Flatpak installed:
   ```bash
   # Ubuntu/Debian
   sudo apt install flatpak

   # Fedora
   sudo dnf install flatpak

   # Arch Linux
   sudo pacman -S flatpak

   # openSUSE
   sudo zypper install flatpak
   ```

2. **Add Flathub repository** (if not already added):
   ```bash
   flatpak remote-add --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo
   ```

3. **Clone and build**:
   ```bash
   git clone https://github.com/tobagin/karere-vala.git
   cd karere-vala
   ./scripts/build.sh --dev --install
   ```

4. **Launch the application**:
   ```bash
   flatpak run io.github.tobagin.karere.Devel
   ```

### Method 2: Build from Source

For advanced users who want to build natively or contribute to development.

#### Install Dependencies

**Ubuntu/Debian:**
```bash
sudo apt update
sudo apt install \
  valac \
  meson \
  build-essential \
  libgtk-4-dev \
  libadwaita-1-dev \
  libwebkitgtk-6.0-dev \
  blueprint-compiler \
  gettext \
  desktop-file-utils \
  appstream-util
```

**Fedora:**
```bash
sudo dnf install \
  vala \
  meson \
  gcc \
  gtk4-devel \
  libadwaita-devel \
  webkitgtk6.0-devel \
  blueprint-compiler \
  gettext \
  desktop-file-utils \
  libappstream-glib
```

**Arch Linux:**
```bash
sudo pacman -S \
  vala \
  meson \
  gcc \
  gtk4 \
  libadwaita \
  webkitgtk-6.0 \
  blueprint-compiler \
  gettext \
  desktop-file-utils \
  appstream-glib
```

#### Build and Install

1. **Clone the repository**:
   ```bash
   git clone https://github.com/tobagin/karere-vala.git
   cd karere-vala
   ```

2. **Configure the build**:
   ```bash
   meson setup build --prefix=/usr/local
   ```

3. **Compile the application**:
   ```bash
   meson compile -C build
   ```

4. **Run tests** (optional but recommended):
   ```bash
   meson test -C build
   ```

5. **Install**:
   ```bash
   sudo meson install -C build
   ```

6. **Launch**:
   ```bash
   karere
   ```

## First-Time Setup

### 1. Initial Launch

When you first launch Karere:

1. The application window will open with a clean LibAdwaita interface
2. WhatsApp Web will begin loading automatically
3. You'll see the familiar WhatsApp Web login screen

### 2. WhatsApp Account Setup

1. **QR Code Method** (Default):
   - Open WhatsApp on your phone
   - Go to **Settings** > **Linked Devices**
   - Tap **Link a Device**
   - Point your phone camera at the QR code in Karere
   - Wait for the connection to establish

2. **Phone Number Method** (Alternative):
   - Click "Link with phone number instead"
   - Enter your phone number
   - Follow the verification steps sent to your phone

### 3. Desktop Integration

Once logged in, Karere will automatically:
- Register for native desktop notifications
- Integrate with your system theme
- Save your login session for future use

### 4. Initial Configuration

It's recommended to review the preferences after first login:

1. **Open Preferences**: Press `Ctrl+,` or use the application menu
2. **Configure Appearance**: Choose your preferred theme (Light/Dark/System)
3. **Set up Notifications**: Configure notification preferences
4. **Privacy Settings**: Review logging and crash reporting options

## Basic Usage

### Main Window

The Karere main window consists of:

- **Header Bar**: Contains window controls and application menu
- **WhatsApp Web Interface**: The full WhatsApp Web experience
- **Status Indicators**: Connection status and notifications

### Essential Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Ctrl+,` | Open Preferences |
| `Ctrl+Q` | Quit Application |
| `F11` | Toggle Fullscreen |
| `Ctrl+R` | Reload WhatsApp Web |
| `Ctrl+Shift+I` | Open Developer Tools (if enabled) |

### Notification System

Karere provides native desktop notifications that:
- Appear as system notifications (not browser notifications)
- Respect your desktop's Do Not Disturb settings
- Support notification actions (when available)
- Can be configured in the Preferences

### Theme Integration

Karere automatically follows your system theme, but you can override this:
- **System Theme**: Automatically follows system Dark/Light preference
- **Light Theme**: Always use light appearance
- **Dark Theme**: Always use dark appearance

## Getting Help

### In-Application Help
- **Application Menu**: Access common actions and preferences
- **About Karere**: View version information and credits
- **Keyboard Shortcuts**: Quick reference guide

### Documentation
- **User Guide**: Comprehensive feature documentation
- **Troubleshooting**: Solutions for common issues
- **FAQ**: Frequently asked questions

### Community Support
- **GitHub Issues**: Report bugs and request features
- **GitHub Discussions**: Community support and questions
- **Project Wiki**: Additional documentation and tips

## Next Steps

Now that Karere is installed and configured:

1. **Explore Features**: Learn about [all available features](features.md)
2. **Customize Preferences**: Review the [preferences guide](preferences.md)
3. **Learn Shortcuts**: Master the [keyboard shortcuts](keyboard-shortcuts.md)
4. **Troubleshooting**: If you encounter issues, check the [troubleshooting guide](../troubleshooting/common-issues.md)

## Quick Tips

- **Window State**: Karere remembers your window size and position
- **Background Running**: The app can run in background for notifications
- **Multiple Accounts**: Each Flatpak instance supports one WhatsApp account
- **Updates**: Flatpak versions update automatically through your system's update mechanism
- **Backup**: Your chat data is stored by WhatsApp, not locally in Karere

Enjoy using Karere for a better WhatsApp Web experience on Linux!
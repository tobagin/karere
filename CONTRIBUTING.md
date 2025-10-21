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

- **Fedora/RHEL-based systems**:
  ```bash
  sudo dnf install flatpak flatpak-builder
  ```

- **Ubuntu/Debian-based systems**:
  ```bash
  sudo apt install flatpak flatpak-builder
  ```

### Building the Development Version

```bash
# Build and install the development Flatpak
./scripts/build.sh --dev

# Run the development version
flatpak run io.github.tobagin.karere.Devel
```

### Local Development (without Flatpak)

For faster iteration during development:

```bash
# Install required dependencies (GNOME SDK)
flatpak install org.gnome.Sdk//49 org.gnome.Platform//49

# Setup build directory
meson setup build -Dprofile=development

# Compile
meson compile -C build

# Run locally
./build/src/karere
```

## Making Changes

### Branch Naming

Create descriptive branch names:
- `feature/add-dark-mode-toggle` - For new features
- `fix/notification-crash` - For bug fixes
- `refactor/simplify-webview-code` - For refactoring
- `docs/update-readme` - For documentation changes

### Commit Messages

Follow conventional commit format:

```
type(scope): brief description

Detailed explanation of the change (optional)

Fixes #issue_number
```

**Types:**
- `feat`: New feature
- `fix`: Bug fix
- `refactor`: Code refactoring
- `docs`: Documentation changes
- `test`: Adding or updating tests
- `chore`: Build process or auxiliary tool changes
- `style`: Code style changes (formatting, etc.)

**Examples:**
```
feat(notifications): add custom sound support

Users can now select custom notification sounds from their system.

Fixes #123
```

```
fix(webview): prevent crash on invalid URLs

Added URL validation before loading to prevent crashes.
```

## Coding Standards

### Vala Code Style

- **Indentation**: 4 spaces (no tabs)
- **Braces**: K&R style (opening brace on same line)
- **Naming Conventions**:
  - Classes: `PascalCase` (e.g., `MainWindow`)
  - Methods: `snake_case` (e.g., `load_webview`)
  - Constants: `UPPER_SNAKE_CASE` (e.g., `MAX_RETRY_COUNT`)
  - Private members: prefix with underscore `_` (e.g., `_webview`)

**Example:**
```vala
public class PreferencesWindow : Adw.PreferencesWindow {
    private Gtk.Switch _dark_mode_switch;
    private const int DEFAULT_TIMEOUT = 5000;

    public void setup_ui() {
        var page = new Adw.PreferencesPage();
        // ...
    }
}
```

### Blueprint UI Files

- Use consistent indentation (2 spaces)
- Keep files focused and modular
- Add comments for complex UI structures

### Flatpak Manifests

- Use YAML format for manifests
- Include comments explaining non-obvious configuration
- Keep modules organized and well-documented

## Testing

### Running Tests

```bash
# Run all tests
meson test -C build

# Run specific test
meson test -C build test_name

# Run with verbose output
meson test -C build --verbose
```

### Manual Testing Checklist

Before submitting:
- [ ] Application builds without errors
- [ ] Application launches successfully
- [ ] No new warnings in console
- [ ] Feature works as expected
- [ ] No regressions in existing features
- [ ] UI is responsive and accessible
- [ ] Works with both light and dark themes

## Submitting Changes

### Pull Request Process

1. **Update your fork**:
   ```bash
   git fetch upstream
   git rebase upstream/main
   ```

2. **Push your changes**:
   ```bash
   git push origin feature/your-feature-name
   ```

3. **Create a Pull Request** on GitHub with:
   - Clear title describing the change
   - Detailed description of what changed and why
   - Reference to related issues (Fixes #123)
   - Screenshots for UI changes

### Pull Request Checklist

- [ ] Code follows project style guidelines
- [ ] Commits are well-formatted and descriptive
- [ ] All tests pass
- [ ] Documentation updated (if needed)
- [ ] No merge conflicts with main branch
- [ ] PR description clearly explains the changes

## Reporting Issues

### Bug Reports

When reporting bugs, include:

1. **Environment Information**:
   - Karere version
   - OS and version
   - Flatpak version
   - Desktop environment (GNOME, KDE, etc.)

2. **Steps to Reproduce**:
   - What you did
   - What you expected to happen
   - What actually happened

3. **Logs** (if applicable):
   ```bash
   flatpak run io.github.tobagin.karere 2>&1 | tee karere.log
   ```

4. **Screenshots** (for UI issues)

### Example Bug Report

```markdown
**Environment:**
- Karere: 1.0.0
- OS: Fedora 40
- Desktop: GNOME 47
- Flatpak: 1.15.6

**Description:**
Notification sound doesn't play when receiving a message.

**Steps to Reproduce:**
1. Open Karere
2. Receive a WhatsApp message
3. Observe that no sound plays

**Expected:** Notification sound should play
**Actual:** No sound plays

**Logs:**
[Attach karere.log file]
```

## Feature Requests

For feature requests:

1. **Search existing issues** to avoid duplicates
2. **Describe the feature** clearly and concisely
3. **Explain the use case** - why is this feature needed?
4. **Provide examples** of how the feature would work
5. **Consider alternatives** - are there other ways to achieve the same goal?

## Architecture Overview

Karere is built with:

- **Language**: Vala (primary), with Blueprint for UI
- **Toolkit**: GTK4 + LibAdwaita for native GNOME look
- **Web Engine**: WebKitGTK 6.0 for WhatsApp Web rendering
- **Build System**: Meson + Ninja
- **Distribution**: Flatpak (sandboxed)

### Project Structure

```
karere/
├── data/               # Application resources
│   ├── icons/          # Application icons
│   ├── screenshots/    # Screenshots for README
│   └── ui/             # Blueprint UI files
├── docs/               # Documentation
├── src/                # Source code
│   ├── application.vala
│   ├── main-window.vala
│   ├── preferences-window.vala
│   └── ...
├── packaging/          # Flatpak manifests
├── scripts/            # Build and utility scripts
├── tests/              # Test files
└── meson.build         # Build configuration
```

## Getting Help

- **Discussions**: Use [GitHub Discussions](https://github.com/tobagin/karere/discussions) for questions
- **Issues**: Use [GitHub Issues](https://github.com/tobagin/karere/issues) for bugs and feature requests
- **Documentation**: Check the [docs/](docs/) directory

## License

By contributing to Karere, you agree that your contributions will be licensed under the GPL-3.0-or-later license.

---

Thank you for contributing to Karere!

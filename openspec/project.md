# Project Context

## Purpose
Karere is a modern, native GTK4/LibAdwaita wrapper for WhatsApp Web that provides seamless integration with the Linux desktop environment. The project aims to deliver a production-ready, native desktop experience for WhatsApp Web with enhanced performance, accessibility, and integration compared to browser-based solutions.

**Key Goals:**
- Provide a native Linux desktop client for WhatsApp Web
- Deliver superior performance through compiled Vala code
- Ensure comprehensive accessibility support for all users
- Maintain privacy-first design with optional telemetry
- Achieve Flathub compliance and sandboxed security

## Tech Stack

### Core Technologies
- **Vala** - Primary programming language (compiled to C)
- **GTK4** (>=4.10.0) - Modern UI toolkit with Wayland support
- **LibAdwaita** (>=1.8.0) - GNOME design language and components (including AdwShortcutsDialog)
- **WebKitGTK 6.0** (>=2.40.0) - Web rendering engine
- **Blueprint** - Declarative UI definition language

### Build & Distribution
- **Meson** (>=0.60.0) - Build system
- **Flatpak** - Application distribution and sandboxing
- **GNOME Platform 49** - Runtime environment

### Supporting Libraries
- **LibSoup 3.0** - HTTP client library
- **GIO/GLib 2.0** (>=2.70.0) - Core system libraries
- **json-glib-1.0** (>=1.6.0) - JSON parsing
- **libgee-0.8** (>=0.20.0) - Collection library

### Development Tools
- **blueprint-compiler** - UI compilation
- **appstreamcli** - Metadata validation
- **desktop-file-validate** - Desktop entry validation
- **Python 3** - Build configuration testing

## Project Conventions

### Code Style

**Vala Conventions:**
- Class names: PascalCase (e.g., `NotificationManager`, `WebkitManager`)
- Method names: snake_case (e.g., `setup_actions()`, `initialize_settings()`)
- Private fields: snake_case with nullable suffix when appropriate (e.g., `private Window? main_window = null`)
- Constants: SCREAMING_SNAKE_CASE (defined in Config namespace)
- Namespace: All code in `Karere` namespace
- Copyright header: Include GPL-3.0-or-later license header in all source files

**File Organization:**
- One class per file (e.g., `application.vala`, `window.vala`)
- Template files use `.in` extension (e.g., `config.vala.in`, `preferences.blp.in`)
- UI files in Blueprint format (`.blp`) compiled to GTK UI (`.ui`)

**Indentation & Formatting:**
- Indentation: 4 spaces (no tabs)
- Brace style: Opening brace on same line
- Target GLib version: 2.78
- Use `debug()` for logging messages

**Comments:**
- Use `//` for single-line comments
- Use `/* */` for multi-line comments and copyright headers
- Include descriptive comments for complex logic
- Add debug messages for important state changes

### Architecture Patterns

**Application Structure:**
- **MVC-inspired separation:** Application → Window → Components
- **Manager pattern:** Specialized managers for distinct concerns (NotificationManager, AccessibilityManager, KeyboardShortcuts, WebkitManager)
- **Settings-driven:** GSettings for persistent configuration
- **Resource-based UI:** GResource compilation for UI templates and icons

**Key Architectural Decisions:**
1. **Deferred initialization:** Settings initialized in `startup()` after GTK initialization
2. **Signal-based communication:** GObject signals for component interaction
3. **Preprocessor directives:** Use `-D DEVELOPMENT` flag for conditional compilation instead of template substitution
4. **Blueprint UI compilation:** Declarative UI files compiled at build time
5. **Icon theme resources:** Custom icons registered via resource paths

**Component Responsibilities:**
- `Application`: Application lifecycle, settings, managers
- `Window`: Main UI container, WebView integration
- `WebkitManager`: WebKit configuration and web content handling
- `NotificationManager`: Desktop notification integration
- `AccessibilityManager`: Screen reader and accessibility support
- `KeyboardShortcuts`: Keyboard shortcut handling
- `Preferences`: Settings UI dialog

### Testing Strategy

**Test Framework:**
- Vala unit tests using GLib test framework
- Python tests for build configuration validation
- Test runner: `test_runner.vala`

**Test Organization:**
- Test files: `tests/test_*.vala`
- One test class per component (e.g., `test_application.vala`)
- Setup/teardown methods for test initialization
- Signal verification for async operations

**Test Execution:**
- Run with: `meson test -C build`
- Test suites: `validation` (metadata/desktop file), `unit-tests`
- Enable tests: `-Dtests=true` (default enabled)

**Validation Tests:**
- Desktop file validation (`desktop-file-validate`)
- AppStream metadata validation (`appstreamcli validate --no-net`)
- Build configuration validation (Python)

**Test Coverage Goals:**
- Core application lifecycle
- Window management
- Notification system
- Settings persistence
- Manager initialization

### Git Workflow

**Branching Strategy:**
- Main branch: `main`
- No separate development branch (direct commits to main)
- Feature branches for major changes

**Commit Conventions:**
Use Gitmoji-style prefixes for semantic commits:
- `✨` feat: New features
- `🐛` fix: Bug fixes
- `♻️` refactor: Code refactoring
- `🗑️` remove: Code removal
- `🧹` cleanup: Code cleanup
- `📝` docs: Documentation changes
- `🔧` chore: Build/config changes

**Examples:**
```
✨ Release v0.9.0 - Enhanced emoji support and rendering improvements
🐛 Release v0.9.4 - Runtime update and bug fixes
♻️ Major refactor: Remove custom logging and crash reporting systems
🗑️ Remove unused SVG icon file
```

**Release Tags:**
- Version format: `v0.9.4`
- Update version in: `meson.build`, metainfo.xml releases
- Tag format matches version number

## Domain Context

**WhatsApp Web Integration:**
- Karere wraps WhatsApp Web (web.whatsapp.com) in a native application
- No WhatsApp API access - relies on web interface
- Uses WebKitGTK to render WhatsApp Web
- Persistent cookie storage for session management
- User agent: Safari Linux for native WebKit experience

**GNOME Ecosystem:**
- Follows GNOME HIG (Human Interface Guidelines)
- Uses LibAdwaita design patterns and components
- Integrates with GNOME desktop features (notifications, accessibility)
- Supports system theme preferences (light/dark/auto)

**Flatpak Sandboxing:**
- Network access required for WhatsApp Web
- Download folder access for file sharing
- Desktop notification permissions
- Hardware acceleration (DRI) for rendering
- Flatpak portal for external link handling

**Accessibility Requirements:**
- Screen reader support (Orca compatibility)
- Keyboard navigation throughout
- ARIA labels for UI components
- Focus management for web content

## Important Constraints

**Technical Constraints:**
- Vala language limitations (no async/await for all operations)
- WebKitGTK sandboxing restrictions
- Flatpak permission model
- GTK4 Wayland-first approach (X11 fallback)
- GSettings schema registration requirements

**Platform Requirements:**
- Linux only (GNOME Platform runtime)
- Minimum display size: 768px
- GTK4 and LibAdwaita versions must stay compatible with GNOME Platform 49
- WebKitGTK 6.0 for modern web standards

**Build Constraints:**
- Meson build system required
- Blueprint compiler for UI files
- Template substitution for app ID/name (development vs. production)
- Icon naming must match app ID at install time

**Distribution Constraints:**
- Flathub requirements for metadata quality
- AppStream validation must pass
- Desktop file validation required
- No external services (privacy requirement)

**Licensing:**
- GPL-3.0-or-later for all code
- CC0-1.0 for metadata
- Must respect WhatsApp Terms of Service

## External Dependencies

**Core Platform:**
- GNOME Platform 49 runtime (Flatpak)
- org.gnome.Sdk for build environment
- org.freedesktop.Platform.ffmpeg-full for media codecs

**System Services:**
- `org.freedesktop.Notifications` - Desktop notifications
- `org.freedesktop.portal.OpenURI` - External link handling (Flatpak)

**External Content:**
- WhatsApp Web (web.whatsapp.com) - The wrapped web application
- Noto Color Emoji fonts - Emoji rendering (bundled in Flatpak)

**Build-time Dependencies:**
- blueprint-compiler (custom compiled in Flatpak)
- Vala compiler (from GNOME SDK)
- Meson build system

**No External APIs:**
- No telemetry services (privacy-first design)
- No crash reporting services (removed in v0.9.4)
- No analytics or tracking
- All functionality is local or through WhatsApp Web

**Icon Themes:**
- Uses GTK icon theme with fallbacks
- Custom symbolic icons for app-specific features
- Standard GNOME icon names for common actions

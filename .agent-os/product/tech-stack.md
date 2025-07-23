# Technical Stack

> Last Updated: 2025-01-23
> Version: 1.0.0

## Core Technologies

### Runtime Environment
- **Platform:** Flatpak
- **SDK:** `org.gnome.Sdk`
- **Version:** `48`

### Application Framework
- **Language:** Vala
- **Framework:** GTK4
- **Framework Version:** `4.19+`
- **UI Library:** LibAdwaita
- **UI Library Version:** `1.7+`
- **Web Engine:** WebKitGTK
- **Web Engine Version:** `6.0`

## Application Architecture

### Data Persistence
- **Database:** SQLite
- **Version:** `3.50+`
- **ORM:** Vala-ORM
- **Storage:** Local file in user data directory (`~/.var/app/io.github.tobagin.karere/data/`)

### State Management
- **Pattern:** GObject properties and data binding. For complex state, custom singleton objects or services are used.

### Asynchronous Operations
- **Method:** Vala's `async/await` syntax with GLib's `GTask` for background work (e.g., networking, file I/O).

### Networking
- **HTTP Client:** `libsoup3`
- **Data Parsing:** `json-glib` for JSON serialization and deserialization.
- **Web Content:** WebKitGTK 6.0 for WhatsApp Web rendering and interaction

## Development Toolchain

### Build System
- **Tool:** Meson
- **Version:** 1.8.0+
- **Dependency Management:** Flatpak manifest, Meson subprojects, or system libraries.

### UI Definition
- **Language:** Blueprint (`.blp` files)
- **Compiler:** `blueprint-compiler`
- **Compiler Version:** `0.18.0+`

### Internationalization (i18n)
- **Framework:** GNU `gettext`
- **Version:** `0.26+`
- **Process:** Strings are marked for translation in Vala and Blueprint files, with `.po` files managed by Meson.

### Code Quality & Documentation
- **Linter:** `vala-lint` for code style enforcement.
- **Version:** latest
- **API Documentation:** `gi-docgen` to generate documentation from source code.

## Assets & Styling

### Styling
- **Framework:** GTK CSS
- **Base Theme:** Adwaita (provided by LibAdwaita)

### Asset Bundling
- **Mechanism:** `GResource`
- **Fonts:** Self-hosted font files (e.g., `.ttf`) bundled via GResource.
- **Icons:** SVG icons (e.g., from Lucide) bundled via GResource.

## Distribution & CI/CD

### Distribution
- **Platform:** Flathub
- **Dev Format:** Flatpak from local source (`io.github.tobagin.karere.Devel`)
- **Prod Format:** Flatpak from git tag (`io.github.tobagin.karere`)

### Flatpak Configuration
- **Production Manifest:** `io.github.tobagin.karere.yml`
- **Development Manifest:** `io.github.tobagin.karere.Devel.yml`
- **Permissions:** Network access for WhatsApp Web, notification access for native notifications

### CI/CD Pipeline
- **Platform:** GitHub Actions
- **Process:** Build and test the Flatpak bundle on push to main/beta branches.
- **Tests:** Unit tests run via `meson test`.

### Environments
- **Production:** `stable` channel on Flathub (built from `main` branch).
- **Staging/Testing:** `beta` channel on Flathub (built from a `beta` or development branch).

## Karere-Specific Components

### WebKitGTK Integration
- **Purpose:** Render WhatsApp Web interface within native GTK application
- **Version:** WebKitGTK 6.0 for modern web standards support
- **Configuration:** Custom user agent, cookie handling, and JavaScript injection for notification interception

### Notification System
- **Native Integration:** GNotification for desktop notification display
- **Web Integration:** JavaScript injection to intercept WhatsApp Web notifications
- **Customization:** Native notification styling and action handling

### Logging Infrastructure
- **Framework:** GLib structured logging (`g_log_structured()`)
- **Levels:** Debug, Info, Warning, Error
- **Storage:** Log files in application data directory
- **Rotation:** Automatic log rotation to prevent disk space issues

### Crash Reporting
- **Detection:** Signal handlers for crash detection
- **Collection:** Stack traces and application state capture
- **Submission:** Opt-in user consent with secure upload mechanism
- **Privacy:** Anonymized data collection with user control
# Product Roadmap

> Last Updated: 2025-01-23
> Version: 1.0.0
> Status: Planning

## Phase 1: Foundation & Core Integration (2-3 weeks)

**Goal:** Establish basic application structure with WebKitGTK integration
**Success Criteria:** Application launches, loads WhatsApp Web, displays in native window

### Must-Have Features

- [ ] Basic Vala application with AdwApplicationWindow - `M`
- [ ] WebKitGTK integration with WhatsApp Web loading - `L`
- [ ] LibAdwaita HeaderBar with basic window controls - `S`
- [ ] Meson build system configuration - `M`
- [ ] Basic error handling and application lifecycle - `M`

### Should-Have Features

- [ ] Custom user agent for WhatsApp Web compatibility - `S`
- [ ] Basic window state persistence (size, maximized) - `M`

### Dependencies

- WebKitGTK 6.0 SDK setup
- Flatpak development environment

## Phase 2: Native Integration & Notifications (2-3 weeks)

**Goal:** Implement native desktop notifications and theme integration
**Success Criteria:** WhatsApp notifications display as native desktop notifications, app respects system theme

### Must-Have Features

- [ ] JavaScript injection for notification interception - `L`
- [ ] GNotification integration for native notifications - `L`
- [ ] LibAdwaita theme support (Light, Dark, System) - `M`
- [ ] Notification permission handling - `M`
- [ ] Basic preferences dialog with theme selector - `L`

### Should-Have Features

- [ ] Notification sound integration - `S`
- [ ] Notification action buttons (reply, mark as read) - `XL`

### Dependencies

- WebKitGTK JavaScript execution
- GNotification system integration

## Phase 3: Logging & Observability (1-2 weeks)

**Goal:** Implement comprehensive logging and debugging capabilities
**Success Criteria:** All application events logged, structured logging output available

### Must-Have Features

- [ ] GLib structured logging implementation - `M`
- [ ] Log level configuration (Debug, Info, Warning, Error) - `S`
- [ ] Log file rotation and management - `M`
- [ ] Debug logging toggle in preferences - `S`

### Should-Have Features

- [ ] Log viewer dialog for users - `L`
- [ ] Export logs functionality - `M`

### Dependencies

- GLib logging framework
- File system permissions for log storage

## Phase 4: Crash Reporting & Stability (2 weeks)

**Goal:** Implement crash detection and reporting system
**Success Criteria:** Crashes detected, reports generated, opt-in submission working

### Must-Have Features

- [ ] Signal handlers for crash detection - `M`
- [ ] Stack trace capture and storage - `L`
- [ ] Crash report dialog with opt-in submission - `L`
- [ ] Privacy controls for crash reporting - `M`

### Should-Have Features

- [ ] Crash report upload to remote service - `XL`
- [ ] Application restart after crash - `M`

### Dependencies

- Signal handling implementation
- Network connectivity for report submission

## Phase 5: Polish & Distribution (2-3 weeks)

**Goal:** Prepare application for production release and distribution
**Success Criteria:** Flatpak manifests ready, application meets GNOME HIG, ready for Flathub

### Must-Have Features

- [x] Production Flatpak manifest (io.github.tobagin.karere.yml) - `M`
- [x] Development Flatpak manifest (io.github.tobagin.karere.Devel.yml) - `M`
- [x] AppData metadata file with screenshots - `M`
- [x] Desktop entry file - `S`
- [x] Application icons (16x16 to 512x512) - `L`

### Should-Have Features

- [ ] Keyboard shortcuts and accessibility improvements - `L`
- [x] About dialog with application information - `S`
- [ ] Help documentation - `M`
- [ ] CI/CD pipeline for automated builds - `XL`

### Dependencies

- Flatpak build environment
- Icon design and creation
- AppStream metadata validation

## Effort Scale Reference

- **XS:** 1 day
- **S:** 2-3 days  
- **M:** 1 week
- **L:** 2 weeks
- **XL:** 3+ weeks
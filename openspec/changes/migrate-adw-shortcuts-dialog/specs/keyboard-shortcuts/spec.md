# Spec: Keyboard Shortcuts Integration

## MODIFIED Requirements

### Requirement: Shortcuts Dialog Presentation
The keyboard shortcuts manager MUST present the new AdwShortcutsDialog when the help overlay action is triggered.

#### Scenario: Help overlay action presents shortcuts dialog
**Given** the KeyboardShortcuts class has set up the show-help-overlay action
**When** the user triggers Ctrl+? or Ctrl+/
**Then** the action must call `show_shortcuts_window()`
**And** `show_shortcuts_window()` must create a new ShortcutsWindow instance
**And** it must call `present(main_window)` on the shortcuts dialog
**And** the dialog must appear centered on the main window

**Previous Behavior**: Same presentation mechanism, different underlying widget
**New Behavior**: Presents AdwShortcutsDialog instead of custom Adw.Dialog

#### Scenario: Shortcuts window instantiation unchanged
**Given** the `show_shortcuts_window()` method in KeyboardShortcuts
**When** creating and presenting the shortcuts dialog
**Then** the code must remain: `var shortcuts_window = new ShortcutsWindow(main_window);`
**And** the code must remain: `shortcuts_window.present(main_window);`
**And** no changes are required to the calling code

**Rationale**: API compatibility is maintained through consistent class interface

### Requirement: Help Overlay Accelerator
The application MUST bind Ctrl+? to show the shortcuts dialog.

#### Scenario: Help overlay keyboard shortcut registered
**Given** the application is setting up keyboard shortcuts
**When** the keyboard-shortcuts-enabled setting is true
**Then** the accelerator for "win.show-help-overlay" must include "<primary>question"
**And** the accelerator for "win.show-help-overlay" must include "<primary>slash"
**And** both Ctrl+? and Ctrl+/ must open the shortcuts dialog

**Previous Behavior**: Same accelerators
**New Behavior**: No change (maintained for consistency)

## ADDED Requirements

### Requirement: Build System Integration
The build system MUST compile the shortcuts dialog Blueprint file and include it in resources.

#### Scenario: Blueprint file compiled at build time
**Given** the meson.build file
**When** configuring blueprint compilation
**Then** `data/ui/shortcuts-dialog.blp` must be included in the blueprint sources list
**And** it must be compiled to `shortcuts-dialog.ui`
**And** the compilation must occur before resource bundle generation

#### Scenario: Shortcuts dialog UI in GResource bundle
**Given** the gresource.xml file (or meson resource configuration)
**When** bundling application resources
**Then** the compiled `shortcuts-dialog.ui` file must be included
**And** it must be accessible at resource path `/io/github/tobagin/karere/shortcuts-dialog.ui`
**And** the resource must be available at runtime

## No Changes Required

### Requirement: Shortcuts Actions Registration
The keyboard shortcuts action registration remains unchanged.

#### Scenario: All shortcut actions registered correctly
**Given** the KeyboardShortcuts class setup
**When** registering shortcuts
**Then** all existing action registrations must remain unchanged:
- Application actions (preferences, quit, about)
- Window actions (minimize, fullscreen, show-help-overlay)
- WebView zoom actions (zoom-in, zoom-out, zoom-reset)
- Accessibility actions (toggle-high-contrast, toggle-focus-indicators)
- Developer actions (dev-tools, reload, force-reload)
- Notification actions (notifications-toggle, dnd-toggle)
- WhatsApp Web actions (find, search-chats, new-chat, archive-chat, profile)

**Rationale**: This change only affects shortcuts *display*, not shortcuts *registration*

### Requirement: Settings-Based Shortcut Enabling
The keyboard shortcuts enabling/disabling logic remains unchanged.

#### Scenario: Shortcuts enabled based on settings
**Given** various shortcut-related settings
**When** registering keyboard shortcuts
**Then** the conditional logic must remain unchanged:
- WebView zoom shortcuts only when "webview-zoom-enabled" is true
- Accessibility shortcuts only when "accessibility-shortcuts-enabled" is true
- Developer shortcuts only when both "developer-shortcuts-enabled" and "developer-tools-enabled" are true
- Notification shortcuts only when "notification-shortcuts-enabled" is true

**Rationale**: This change only affects shortcuts *display*, not shortcuts *activation*

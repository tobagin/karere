# Spec: UI Shortcuts Dialog

## ADDED Requirements

### Requirement: AdwShortcutsDialog UI Definition
The application MUST provide a shortcuts dialog UI definition using AdwShortcutsDialog widget from LibAdwaita 1.8.

#### Scenario: Shortcuts dialog defined in Blueprint
**Given** the application uses Blueprint for UI definitions
**When** a shortcuts dialog UI file is required
**Then** a `shortcuts-dialog.blp` file must exist in `data/ui/`
**And** it must define an AdwShortcutsDialog with ID "shortcuts_dialog"
**And** the dialog must be compiled to `shortcuts-dialog.ui` at build time
**And** the compiled UI must be included in the application's GResource bundle at path `/io/github/tobagin/karere/shortcuts-dialog.ui`

#### Scenario: All shortcut sections are defined
**Given** the shortcuts dialog Blueprint file
**When** defining the dialog structure
**Then** it must include these sections in order:
- "General" section for application-wide shortcuts
- "Window" section for window management shortcuts
- "WhatsApp Web" section for WhatsApp-specific shortcuts
- "WebView Zoom" section for zoom controls (conditional)
- "Accessibility" section for accessibility features (conditional)
- "Developer" section for development tools (conditional)
- "Notifications" section for notification controls (conditional)

#### Scenario: General section contains application shortcuts
**Given** the "General" shortcuts section
**When** defining shortcuts in this section
**Then** it must include:
- "Show this help" → Ctrl+?
- "Preferences" → Ctrl+,
- "About" → F1
- "Quit application" → Ctrl+Q

#### Scenario: Window section contains window management shortcuts
**Given** the "Window" shortcuts section
**When** defining shortcuts in this section
**Then** it must include:
- "Minimize window" → Ctrl+M
- "Fullscreen" → F11 or Alt+Return

#### Scenario: WhatsApp Web section contains messaging shortcuts
**Given** the "WhatsApp Web" shortcuts section
**When** defining shortcuts in this section
**Then** it must include:
- "Find in chat" → Ctrl+F
- "Search chats" → Ctrl+Shift+F
- "New chat" → Ctrl+N
- "Archive current chat" → Ctrl+E
- "Open profile" → Ctrl+P
- "Reload WhatsApp Web" → F5

#### Scenario: WebView Zoom section contains zoom shortcuts
**Given** the "WebView Zoom" shortcuts section
**When** defining shortcuts in this section
**Then** it must include:
- "Zoom in" → Ctrl++ or Ctrl+Numpad+
- "Zoom out" → Ctrl+- or Ctrl+Numpad-
- "Reset zoom" → Ctrl+0 or Ctrl+Numpad0

#### Scenario: Accessibility section contains accessibility shortcuts
**Given** the "Accessibility" shortcuts section
**When** defining shortcuts in this section
**Then** it must include:
- "Toggle high contrast" → Ctrl+Shift+H
- "Toggle focus indicators" → Ctrl+Shift+F

#### Scenario: Developer section contains development shortcuts
**Given** the "Developer" shortcuts section
**When** defining shortcuts in this section
**Then** it must include:
- "Developer tools" → F12 or Ctrl+Shift+D
- "Reload page" → F5 or Ctrl+R
- "Force reload" → Shift+F5 or Ctrl+Shift+R

#### Scenario: Notifications section contains notification shortcuts
**Given** the "Notifications" shortcuts section
**When** defining shortcuts in this section
**Then** it must include:
- "Toggle notifications" → Ctrl+Shift+N
- "Toggle Do Not Disturb" → Ctrl+Shift+D

### Requirement: Shortcuts Dialog Class Integration
The ShortcutsWindow class MUST load and present the AdwShortcutsDialog from the UI resource.

#### Scenario: Dialog loaded from UI resource
**Given** the ShortcutsWindow class is instantiated
**When** the constructor is called
**Then** it must load the shortcuts dialog from the GResource at `/io/github/tobagin/karere/shortcuts-dialog.ui`
**And** it must use Gtk.Builder to parse the UI definition
**And** it must retrieve the dialog object with ID "shortcuts_dialog"
**And** the retrieved dialog must be stored as the class instance

#### Scenario: Section references obtained for visibility control
**Given** the shortcuts dialog has been loaded from UI
**When** setting up the dialog
**Then** it must obtain references to these sections with their IDs:
- "webview_zoom_section" for WebView Zoom section
- "accessibility_section" for Accessibility section
- "developer_section" for Developer section
- "notifications_section" for Notifications section
**And** these references must be stored as private instance variables

#### Scenario: Constructor signature remains compatible
**Given** existing code calls `new ShortcutsWindow(parent)`
**When** refactoring to use AdwShortcutsDialog
**Then** the constructor signature `public ShortcutsWindow(Gtk.Window parent)` must remain unchanged
**And** the class must still extend `Adw.Dialog`
**And** calling code must not require modifications

### Requirement: Dynamic Section Visibility
The shortcuts dialog MUST show or hide sections based on application settings.

#### Scenario: WebView zoom section visibility
**Given** the WebView Zoom section in the shortcuts dialog
**When** the setting "webview-zoom-enabled" is true
**Then** the WebView Zoom section must be visible
**When** the setting "webview-zoom-enabled" is false
**Then** the WebView Zoom section must be hidden

#### Scenario: Accessibility section visibility
**Given** the Accessibility section in the shortcuts dialog
**When** the setting "accessibility-shortcuts-enabled" is true
**Then** the Accessibility section must be visible
**When** the setting "accessibility-shortcuts-enabled" is false
**Then** the Accessibility section must be hidden

#### Scenario: Developer section visibility
**Given** the Developer section in the shortcuts dialog
**When** the setting "developer-shortcuts-enabled" is true
**And** the setting "developer-tools-enabled" is true
**Then** the Developer section must be visible
**When** either "developer-shortcuts-enabled" is false
**Or** "developer-tools-enabled" is false
**Then** the Developer section must be hidden

#### Scenario: Notifications section visibility
**Given** the Notifications section in the shortcuts dialog
**When** the setting "notification-shortcuts-enabled" is true
**Then** the Notifications section must be visible
**When** the setting "notification-shortcuts-enabled" is false
**Then** the Notifications section must be hidden

#### Scenario: Visibility updates when settings change
**Given** the shortcuts dialog is open
**When** a user changes a shortcut-related setting in preferences
**Then** the relevant section visibility must update immediately
**And** the dialog must not need to be closed and reopened
**And** the update must occur without rebuilding the entire dialog

#### Scenario: Keyboard shortcuts disabled message
**Given** the shortcuts dialog
**When** the setting "keyboard-shortcuts-enabled" is false
**Then** all shortcut sections must be hidden
**And** a message must be displayed stating "Keyboard shortcuts are currently disabled"
**And** the message must instruct users to enable shortcuts in "Preferences → Accessibility"

### Requirement: Settings Listeners for Real-time Updates
The ShortcutsWindow class MUST listen to settings changes and update section visibility accordingly.

#### Scenario: Settings change listeners registered
**Given** the ShortcutsWindow instance is created
**When** setting up the dialog
**Then** it must register change listeners for these settings:
- "keyboard-shortcuts-enabled"
- "webview-zoom-enabled"
- "accessibility-shortcuts-enabled"
- "developer-shortcuts-enabled"
- "developer-tools-enabled"
- "notification-shortcuts-enabled"
**And** each listener must trigger visibility updates when the setting changes

#### Scenario: Visibility update method updates all sections
**Given** a settings change event is triggered
**When** the visibility update method is called
**Then** it must evaluate the current state of all relevant settings
**And** it must update the visibility property of each conditional section
**And** the update must complete synchronously before returning

## MODIFIED Requirements

### Requirement: LibAdwaita Version Requirement
The application's minimum LibAdwaita dependency MUST be updated to 1.8.0 or higher to support AdwShortcutsDialog.

#### Scenario: Minimum LibAdwaita version is 1.8.0
**Given** the meson.build file
**When** declaring the libadwaita-1 dependency
**Then** the version requirement must be `>=1.8.0`
**And** the dependency must be declared as: `dependency('libadwaita-1', version: '>=1.8.0')`

**Previous Behavior**: Required LibAdwaita >= 1.3.0
**New Behavior**: Requires LibAdwaita >= 1.8.0

### Requirement: Project Documentation Version Metadata
The project documentation MUST reflect the updated LibAdwaita version requirement.

#### Scenario: Project.md documents LibAdwaita 1.8 minimum
**Given** the openspec/project.md file
**When** documenting core technologies
**Then** the LibAdwaita entry must specify version `(>=1.8.0)`
**And** it must list AdwShortcutsDialog as a used component

**Previous Behavior**: Documented LibAdwaita (>=1.3.0)
**New Behavior**: Documents LibAdwaita (>=1.8.0) with AdwShortcutsDialog

## REMOVED Requirements

### Requirement: Manual Shortcuts UI Construction
The ShortcutsWindow class SHALL NOT build the shortcuts UI programmatically.

#### Scenario: No programmatic UI building
**Given** the ShortcutsWindow class implementation
**When** reviewing the code
**Then** it must not contain methods for building UI components programmatically
**And** these methods must be removed:
- `create_shortcuts_content()`
- `add_general_shortcuts()`
- `add_window_shortcuts()`
- `add_whatsapp_shortcuts()`
- `add_accessibility_shortcuts()`
- `add_developer_shortcuts()`
- `add_webview_zoom_shortcuts()`
- `add_notification_shortcuts()`
- `add_shortcut_row()`

**Rationale**: UI structure is now defined declaratively in Blueprint file

#### Scenario: No manual dialog setup
**Given** the ShortcutsWindow class implementation
**When** setting up the dialog
**Then** it must not manually create Adw.HeaderBar
**And** it must not manually create Gtk.ScrolledWindow
**And** it must not manually create Adw.PreferencesPage
**And** it must not manually create Adw.PreferencesGroup instances
**And** it must not manually create Adw.ActionRow instances
**And** the method `setup_dialog()` must be removed or significantly simplified

**Rationale**: AdwShortcutsDialog provides all necessary UI structure

#### Scenario: No content refresh mechanism
**Given** the ShortcutsWindow class implementation
**When** handling settings changes
**Then** it must not rebuild the entire shortcuts content
**And** the method `refresh_shortcuts_content()` must be removed
**And** the instance variable `content_page` must be removed
**And** the instance variable `scrolled_window` must be removed

**Rationale**: Visibility toggling replaces content rebuilding

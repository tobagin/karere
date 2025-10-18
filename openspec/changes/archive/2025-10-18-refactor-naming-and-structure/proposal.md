# Proposal: Refactor Naming and Structure

## Why
The current codebase uses inconsistent file naming conventions (lowercase/kebab-case) that don't follow Vala's standard PascalCase convention for filenames. Additionally, all source files are in a flat `src/` directory, making it harder to navigate and understand the codebase organization. Some class names also don't accurately reflect their purpose (e.g., `ShortcutsWindow` is actually a dialog wrapper, `Preferences` should be `PreferencesDialog`).

## What Changes
1. **Rename all .vala files to PascalCase** to match Vala standard conventions
2. **Organize files into logical subdirectories** under `src/`:
   - `dialogs/` - Dialog classes (AboutDialog, PreferencesDialog, ShortcutsDialog)
   - `managers/` - Manager classes (AccessibilityManager, NotificationManager, etc.)
   - `utils/` - Utility classes and constants
   - Root `src/` - Core application files (Application, Window, Main)
3. **Rename classes** to better reflect their purpose:
   - `ShortcutsWindow` → `ShortcutsDialog`
   - `Preferences` → `PreferencesDialog`
   - `WebkitManager` → `WebKitManager` (fix capitalization)
4. **Update project.md** to reflect PascalCase file naming convention
5. **Update build system** (meson.build) to reference new file paths

## Impact
- **Affected files**: All 16 .vala source files in `src/`
- **Affected specs**: Build system configuration, project conventions
- **Breaking changes**: None (internal refactoring only, no API changes)
- **Benefits**:
  - Follows Vala standard conventions (PascalCase filenames)
  - Improved code navigation and discoverability
  - Clearer separation of concerns (dialogs vs managers vs core)
  - More accurate class names that reflect actual functionality
  - Easier onboarding for new contributors familiar with Vala conventions

## File Mapping

### Before → After

**Dialogs:**
- `src/about-dialog.vala` → `src/dialogs/AboutDialog.vala`
- `src/preferences.vala` → `src/dialogs/PreferencesDialog.vala` (class renamed)
- `src/shortcuts-window.vala` → `src/dialogs/ShortcutsDialog.vala` (class renamed)

**Managers:**
- `src/accessibility-manager.vala` → `src/managers/AccessibilityManager.vala`
- `src/keyboard-shortcuts.vala` → `src/managers/KeyboardShortcuts.vala`
- `src/notification-manager.vala` → `src/managers/NotificationManager.vala`
- `src/settings-manager.vala` → `src/managers/SettingsManager.vala`
- `src/webkit-manager.vala` → `src/managers/WebKitManager.vala` (class renamed)
- `src/whatsapp-integration.vala` → `src/managers/WhatsAppIntegration.vala`

**Utils:**
- `src/constants.vala` → `src/utils/Constants.vala`
- `src/dependency-container.vala` → `src/utils/DependencyContainer.vala`
- `src/utils.vala` → `src/utils/Utils.vala`

**Core (root src/):**
- `src/application.vala` → `src/Application.vala`
- `src/config.vala.in` → `src/Config.vala.in`
- `src/main.vala` → `src/Main.vala`
- `src/window.vala` → `src/Window.vala`

## Class Renames

1. **ShortcutsWindow → ShortcutsDialog**
   - Rationale: The class wraps `AdwShortcutsDialog`, not a window
   - File: `src/dialogs/ShortcutsDialog.vala`

2. **Preferences → PreferencesDialog**
   - Rationale: The class extends `Adw.PreferencesDialog`, should be named accordingly
   - File: `src/dialogs/PreferencesDialog.vala`

3. **WebkitManager → WebKitManager**
   - Rationale: Fix capitalization to match WebKit branding
   - File: `src/managers/WebKitManager.vala`

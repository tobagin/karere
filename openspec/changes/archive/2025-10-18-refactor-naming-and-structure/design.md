# Design: Refactor Naming and Structure

## Overview
This refactoring reorganizes the source code to follow Vala standard conventions and improve code organization through logical directory structure.

## Directory Structure

### Before
```
src/
├── about-dialog.vala
├── accessibility-manager.vala
├── application.vala
├── config.vala.in
├── constants.vala
├── dependency-container.vala
├── keyboard-shortcuts.vala
├── main.vala
├── notification-manager.vala
├── preferences.vala
├── settings-manager.vala
├── shortcuts-window.vala
├── utils.vala
├── webkit-manager.vala
├── whatsapp-integration.vala
└── window.vala
```

### After
```
src/
├── Application.vala
├── Config.vala.in
├── Main.vala
├── Window.vala
├── dialogs/
│   ├── AboutDialog.vala
│   ├── PreferencesDialog.vala
│   └── ShortcutsDialog.vala
├── managers/
│   ├── AccessibilityManager.vala
│   ├── KeyboardShortcuts.vala
│   ├── NotificationManager.vala
│   ├── SettingsManager.vala
│   ├── WebKitManager.vala
│   └── WhatsAppIntegration.vala
└── utils/
    ├── Constants.vala
    ├── DependencyContainer.vala
    └── Utils.vala
```

## Naming Convention Rationale

### PascalCase for Filenames

**Decision**: Use PascalCase for all .vala filenames

**Rationale**:
- **Vala Standard**: PascalCase is the conventional naming for Vala source files
- **Class-to-File Mapping**: One class per file with matching names (e.g., `NotificationManager` class in `NotificationManager.vala`)
- **Industry Practice**: Most Vala projects (including GNOME core apps) use PascalCase filenames
- **IDE Support**: Better autocomplete and navigation in IDEs that expect PascalCase
- **Consistency**: Aligns with the existing PascalCase class names

**Trade-offs**:
- Requires git mv operations (history preserved with `--follow`)
- Build system updates needed
- Slightly less "Unix-y" than kebab-case, but more aligned with Vala ecosystem

### Directory Organization

**Decision**: Group files by architectural role

**Categories**:

1. **Root `src/`** - Core application lifecycle
   - `Application.vala` - Application entry point and lifecycle
   - `Window.vala` - Main window and UI container
   - `Main.vala` - Program entry point
   - `Config.vala.in` - Build configuration template

2. **`dialogs/`** - User-facing dialog windows
   - `AboutDialog.vala` - About/credits dialog
   - `PreferencesDialog.vala` - Settings/preferences dialog
   - `ShortcutsDialog.vala` - Keyboard shortcuts help dialog

3. **`managers/`** - Service/manager classes
   - `AccessibilityManager.vala` - Accessibility features
   - `KeyboardShortcuts.vala` - Keyboard shortcut handling
   - `NotificationManager.vala` - Desktop notifications
   - `SettingsManager.vala` - Settings management
   - `WebKitManager.vala` - WebKit configuration
   - `WhatsAppIntegration.vala` - WhatsApp-specific logic

4. **`utils/`** - Utility classes and constants
   - `Constants.vala` - Application constants
   - `DependencyContainer.vala` - Dependency injection
   - `Utils.vala` - Helper functions

**Rationale**:
- **Clear Separation**: Easy to locate files by function
- **Scalability**: New files can be easily categorized
- **Onboarding**: New contributors can quickly understand codebase structure
- **GNOME Pattern**: Similar to organization in other GNOME applications

## Class Rename Rationale

### ShortcutsWindow → ShortcutsDialog

**Current State**:
```vala
public class ShortcutsWindow : GLib.Object {
    private Adw.ShortcutsDialog dialog;
    // ...
}
```

**Problem**: The class wraps `AdwShortcutsDialog` but is named "Window", which is misleading.

**Solution**: Rename to `ShortcutsDialog` to reflect that it provides a dialog interface.

**Impact**:
- Update references in `KeyboardShortcuts.show_shortcuts_window()` → `show_shortcuts_dialog()`
- Update references in `Preferences` dialog shortcuts button handler

### Preferences → PreferencesDialog

**Current State**:
```vala
public class Preferences : Adw.PreferencesDialog {
    // ...
}
```

**Problem**: The class extends `Adw.PreferencesDialog` but is named just "Preferences", which could be ambiguous.

**Solution**: Rename to `PreferencesDialog` for clarity and consistency with other dialog classes.

**Impact**:
- Update references in `Application.show_preferences()`
- Update references in `KeyboardShortcuts.setup_actions()`

### WebkitManager → WebKitManager

**Current State**:
```vala
public class WebKitManager : GLib.Object {
    // ...
}
```

**Problem**: "Webkit" should be "WebKit" to match the official branding and casing.

**Solution**: Rename to `WebKitManager`.

**Impact**:
- Update references in `Window` class
- Update references in `Application` class initialization

## Build System Updates

### Meson Build Configuration

**Files to Update**:
- `meson.build` - Source file list and paths
- `tests/meson.build` - Test file references (if applicable)

**Changes Required**:

```meson
# Before
karere_sources = files(
  'src/main.vala',
  'src/application.vala',
  'src/window.vala',
  'src/preferences.vala',
  // ... etc
)

# After
karere_sources = files(
  'src/Main.vala',
  'src/Application.vala',
  'src/Window.vala',
  'src/dialogs/PreferencesDialog.vala',
  'src/dialogs/AboutDialog.vala',
  'src/dialogs/ShortcutsDialog.vala',
  'src/managers/AccessibilityManager.vala',
  'src/managers/KeyboardShortcuts.vala',
  'src/managers/NotificationManager.vala',
  'src/managers/SettingsManager.vala',
  'src/managers/WebKitManager.vala',
  'src/managers/WhatsAppIntegration.vala',
  'src/utils/Constants.vala',
  'src/utils/DependencyContainer.vala',
  'src/utils/Utils.vala',
)
```

## Migration Strategy

### Phase 1: Create Directory Structure
1. Create `src/dialogs/`, `src/managers/`, `src/utils/` directories
2. Verify directories are created successfully

### Phase 2: Move and Rename Files
1. Use `git mv` to preserve history
2. Move files to appropriate directories with PascalCase names
3. Handle special case: `config.vala.in` → `Config.vala.in`

### Phase 3: Update File Contents
1. Rename classes: `ShortcutsWindow` → `ShortcutsDialog`
2. Rename classes: `Preferences` → `PreferencesDialog`
3. Rename classes: `WebkitManager` → `WebKitManager`

### Phase 4: Update References
1. Update all imports/references to renamed classes
2. Update method names (e.g., `show_shortcuts_window()` → `show_shortcuts_dialog()`)
3. Update build system file paths

### Phase 5: Update Documentation
1. Update `openspec/project.md` file naming convention
2. Update any code comments referencing old names
3. Update README if it references file structure

## Testing Considerations

### Build Validation
- Verify clean build: `meson setup build --wipe && ninja -C build`
- Ensure all files are found and compiled
- Check for missing file errors

### Runtime Validation
- Launch application and verify it starts
- Open each dialog (preferences, shortcuts, about)
- Verify all managers initialize correctly
- Test keyboard shortcuts still work

### Git History
- Verify `git log --follow` preserves file history
- Check that git blame works correctly
- Ensure file moves are detected by git

## Rollback Plan

If issues are discovered:
1. Revert build system changes in `meson.build`
2. Use `git mv` to move files back to original locations
3. Revert class renames in source files
4. Rebuild and test

## Documentation Updates

### Files Requiring Updates

1. **openspec/project.md**
   - Update "File Organization" section to specify PascalCase
   - Update example file names
   - Add directory structure documentation

2. **Code Comments**
   - Update any TODOs or comments referencing old class/file names

3. **README.md** (if applicable)
   - Update any file structure documentation
   - Update contributing guidelines if they reference file naming

## Performance Considerations

- **Build Time**: No impact (same number of files, just reorganized)
- **Runtime**: Zero impact (internal refactoring only)
- **Git Operations**: Slightly larger git operation due to file moves, but history preserved

## Future Benefits

This refactoring sets the foundation for:
- **Better Scalability**: Easy to add new managers or dialogs
- **Clearer Architecture**: New contributors can quickly understand structure
- **Maintainability**: Related files grouped together
- **Standards Compliance**: Follows Vala and GNOME conventions

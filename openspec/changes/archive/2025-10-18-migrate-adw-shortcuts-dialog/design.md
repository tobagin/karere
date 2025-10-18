# Design: Migrate to AdwShortcutsDialog

## Architecture Overview

This change migrates from a custom shortcuts display implementation to the standard `AdwShortcutsDialog` widget introduced in LibAdwaita 1.8.

## Component Structure

### Before (Custom Implementation)
```
ShortcutsWindow (extends Adw.Dialog)
├── Gtk.Box (vertical)
│   ├── Adw.HeaderBar
│   └── Gtk.ScrolledWindow
│       └── Adw.PreferencesPage (dynamically built)
│           ├── Adw.PreferencesGroup (General)
│           │   └── Adw.ActionRow × N (shortcuts)
│           ├── Adw.PreferencesGroup (Window)
│           ├── Adw.PreferencesGroup (WhatsApp Web)
│           ├── Adw.PreferencesGroup (Accessibility) [conditional]
│           ├── Adw.PreferencesGroup (Developer) [conditional]
│           ├── Adw.PreferencesGroup (WebView Zoom) [conditional]
│           └── Adw.PreferencesGroup (Notifications) [conditional]
```

### After (Standard Widget)
```
AdwShortcutsDialog (loaded from UI file)
├── AdwShortcutsSection (General)
│   └── AdwShortcutsItem × N
├── AdwShortcutsSection (Window)
│   └── AdwShortcutsItem × N
├── AdwShortcutsSection (WhatsApp Web)
│   └── AdwShortcutsItem × N
├── AdwShortcutsSection (Accessibility) [conditional visibility]
│   └── AdwShortcutsItem × N
├── AdwShortcutsSection (Developer) [conditional visibility]
│   └── AdwShortcutsItem × N
├── AdwShortcutsSection (WebView Zoom) [conditional visibility]
│   └── AdwShortcutsItem × N
└── AdwShortcutsSection (Notifications) [conditional visibility]
    └── AdwShortcutsItem × N
```

## Key Design Decisions

### 1. Blueprint vs Programmatic Construction

**Decision**: Use Blueprint UI files

**Rationale**:
- AdwShortcutsDialog is designed to be loaded from UI files (similar to Gtk.ShortcutsWindow)
- Blueprint provides clearer, more maintainable declarative syntax
- Project already uses Blueprint for other UI components (window.blp, preferences.blp.in)
- Easier to visualize and modify structure
- Consistent with project conventions

**Trade-offs**:
- Less flexibility for dynamic content
- Requires compile-time UI generation
- Must use visibility properties for conditional sections

### 2. Dynamic Content Handling

**Decision**: Use widget visibility properties controlled by settings listeners

**Rationale**:
- Settings can change while dialog is open (user could toggle in preferences)
- Visibility approach allows instant updates without rebuilding entire dialog
- Maintains current behavior where sections appear/disappear based on settings

**Implementation**:
```vala
// Load dialog once from UI file
shortcuts_dialog = builder.get_object("shortcuts_dialog") as Adw.ShortcutsDialog;

// Get section references
accessibility_section = builder.get_object("accessibility_section") as Adw.ShortcutsSection;
developer_section = builder.get_object("developer_section") as Adw.ShortcutsSection;
// ... etc

// Update visibility based on settings
accessibility_section.visible = settings.get_boolean("accessibility-shortcuts-enabled");
developer_section.visible = settings.get_boolean("developer-shortcuts-enabled") &&
                            settings.get_boolean("developer-tools-enabled");
```

**Alternatives Considered**:
- Rebuild entire dialog on settings change: Too expensive, breaks user experience
- Show all sections always: Confusing when shortcuts are disabled
- Single build with no updates: Doesn't reflect settings changes

### 3. Section Organization

**Decision**: Map existing groups 1:1 to AdwShortcutsSection

**Rationale**:
- Maintains familiar organization for existing users
- Logical grouping by functionality area
- Matches current implementation structure

**Sections**:
1. **General** - Always visible
   - Application-wide shortcuts (preferences, about, quit, help)

2. **Window** - Always visible
   - Window management (minimize, fullscreen)

3. **WhatsApp Web** - Always visible
   - WhatsApp-specific shortcuts (find, search, new chat, archive, profile, reload)

4. **WebView Zoom** - Conditional
   - Visible when: `webview-zoom-enabled == true`
   - Zoom in, zoom out, reset

5. **Accessibility** - Conditional
   - Visible when: `accessibility-shortcuts-enabled == true`
   - High contrast, focus indicators

6. **Developer** - Conditional
   - Visible when: `developer-shortcuts-enabled == true && developer-tools-enabled == true`
   - Dev tools, reload, force reload

7. **Notifications** - Conditional
   - Visible when: `notification-shortcuts-enabled == true`
   - Toggle notifications, toggle DND

### 4. AdwApplication Integration

**Decision**: Leverage automatic shortcuts action setup

**Rationale**:
- AdwApplication automatically creates `app.shortcuts` action when it finds `shortcuts-dialog.ui` in resources
- Automatically binds Ctrl+? accelerator
- Less boilerplate code

**Implementation Requirements**:
- UI file must be at resource path: `/io/github/tobagin/karere/shortcuts-dialog.ui`
- Dialog must have ID: `shortcuts_dialog`
- Will replace manual action creation in KeyboardShortcuts class

### 5. Accelerator Display Format

**Decision**: Use standard GTK accelerator syntax in Blueprint

**Rationale**:
- AdwShortcutsItem uses `accelerator` property with GTK syntax
- Automatic parsing and display formatting
- Supports multiple accelerators per shortcut

**Examples**:
```blueprint
AdwShortcutsItem {
  title: _("Preferences");
  accelerator: "<Primary>comma";  // Shows as Ctrl+,
}

AdwShortcutsItem {
  title: _("Fullscreen");
  accelerator: "F11 <Alt>Return";  // Shows as F11 or Alt+Return
}
```

### 6. Settings Listener Pattern

**Decision**: Maintain settings listeners in ShortcutsWindow class

**Rationale**:
- Enables dynamic updates without manual refresh
- Consistent with current implementation
- User sees immediate feedback when toggling shortcuts in preferences

**Implementation**:
```vala
private void setup_settings_listeners() {
    settings.changed["keyboard-shortcuts-enabled"].connect(update_visibility);
    settings.changed["webview-zoom-enabled"].connect(update_visibility);
    settings.changed["accessibility-shortcuts-enabled"].connect(update_visibility);
    settings.changed["developer-shortcuts-enabled"].connect(update_visibility);
    settings.changed["developer-tools-enabled"].connect(update_visibility);
    settings.changed["notification-shortcuts-enabled"].connect(update_visibility);
}

private void update_visibility() {
    // Update section visibility based on current settings
    update_section_visibility();
}
```

### 7. Class Refactoring Approach

**Decision**: Simplify ShortcutsWindow class, keep same interface

**Rationale**:
- Minimal changes to calling code (KeyboardShortcuts, Preferences)
- Constructor signature remains compatible
- Reduced from ~270 lines to ~100 lines
- Eliminates all manual UI building code

**API Compatibility**:
```vala
// Before and After - same interface
public class ShortcutsWindow : Adw.Dialog {
    public ShortcutsWindow(Gtk.Window parent) { ... }
}

// Called from KeyboardShortcuts.show_shortcuts_window()
var shortcuts_window = new ShortcutsWindow(main_window);
shortcuts_window.present(main_window);
```

## Migration Strategy

### Phase 1: Blueprint UI Creation
1. Create `data/ui/shortcuts-dialog.blp`
2. Define all sections and shortcuts statically
3. Add object IDs for sections that need visibility control
4. Test compilation to UI file

### Phase 2: Class Refactoring
1. Update ShortcutsWindow to load from UI resource
2. Get references to conditional sections
3. Implement visibility update logic
4. Remove all manual UI building code

### Phase 3: Integration Updates
1. Update meson.build to compile new Blueprint file
2. Update gresource.xml to include compiled UI
3. Remove dependency on AdwApplication auto-setup (since we present dialog manually)
4. Test all shortcuts display correctly

### Phase 4: Dependency Update
1. Update meson.build: LibAdwaita version from 1.3.0 to 1.8.0
2. Update openspec/project.md with new minimum version
3. Test build with new version requirement

## Testing Considerations

### Unit Testing
- Verify dialog loads from UI file successfully
- Verify all sections are present
- Verify conditional sections respond to settings changes

### Integration Testing
- Test dialog presentation from keyboard shortcut (Ctrl+?)
- Test dialog presentation from preferences view shortcuts button
- Test dialog presentation from KeyboardShortcuts.show_shortcuts_window()
- Test settings changes while dialog is open

### Manual Testing
- Verify all shortcuts are displayed correctly
- Verify accelerator formatting is readable
- Verify sections show/hide based on settings
- Test with different settings combinations
- Verify accessibility (screen reader, keyboard navigation)
- Verify dialog size and scrolling behavior

## Performance Considerations

### Memory
- Dialog loaded once from UI file (not rebuilt on each show)
- Sections remain in memory but hidden when not needed
- Minimal overhead compared to previous implementation

### Startup
- Blueprint compilation happens at build time
- UI loading happens on first dialog creation (lazy)
- No impact on application startup time

### Runtime
- Settings change updates are O(1) visibility toggles
- Much faster than previous approach (rebuilding entire content)

## Accessibility

AdwShortcutsDialog provides built-in accessibility features:
- Proper ARIA labels
- Keyboard navigation
- Screen reader support
- Focus management

These are improvements over the custom implementation which relied on manual accessibility setup.

## Future Enhancements

Potential future improvements enabled by this change:
1. **Search functionality**: AdwShortcutsDialog may add search in future versions
2. **Categorization**: Better visual hierarchy with section grouping
3. **Icons**: Future versions might support icons in shortcuts items
4. **Gestures**: If needed, could potentially add gesture support

## Rollback Plan

If issues are discovered after implementation:
1. Revert meson.build dependency change
2. Restore previous ShortcutsWindow.vala implementation
3. Remove shortcuts-dialog.blp file
4. Remove UI file from gresource.xml

## Dependencies

### Build-time
- blueprint-compiler (already in build dependencies)
- LibAdwaita >= 1.8.0 (new requirement)

### Runtime
- GNOME Platform 49 runtime (already provides LibAdwaita 1.8.0)

## Documentation Updates

Files requiring updates:
- `openspec/project.md` - Update minimum LibAdwaita version to 1.8.0
- Code comments in ShortcutsWindow class
- No user-facing documentation changes (behavior is the same)

# Tasks: Migrate to AdwShortcutsDialog

## Overview
This task list breaks down the migration to AdwShortcutsDialog into small, verifiable work items that deliver incremental progress.

## Dependencies
- Tasks marked with `[BLOCKS]` must complete before dependent tasks can start
- Tasks marked with `[PARALLEL]` can be done concurrently with other parallel tasks

## Task List

### 1. Update Build Dependencies
**Status**: Pending
**Estimated Effort**: 5 minutes
**Parallelizable**: No (blocks subsequent tasks)

Update meson.build to require LibAdwaita 1.8.0:
- Change `dependency('libadwaita-1', version: '>=1.3.0')` to `version: '>=1.8.0'`
- Test build configuration: `meson setup build --wipe` (or reconfigure existing build)
- Verify no errors in dependency resolution

**Validation**:
- [ ] `meson setup build --wipe` completes successfully
- [ ] Build log shows LibAdwaita 1.8.0+ detected
- [ ] No dependency errors or warnings

**Deliverable**: Updated meson.build with LibAdwaita 1.8.0 minimum version

---

### 2. Create Shortcuts Dialog Blueprint File
**Status**: Pending
**Estimated Effort**: 45 minutes
**Parallelizable**: Can start after task 1
**Blocks**: Tasks 4, 5, 6

Create `data/ui/shortcuts-dialog.blp` with complete AdwShortcutsDialog structure:

**2.1**: Create file structure
- Create `data/ui/shortcuts-dialog.blp`
- Add copyright header and license
- Define top-level AdwShortcutsDialog with ID "shortcuts_dialog"

**2.2**: Add General shortcuts section
- Define AdwShortcutsSection with title "General"
- Add shortcut items: Show this help, Preferences, About, Quit application
- Use proper accelerator syntax (e.g., `<Primary>comma`)

**2.3**: Add Window shortcuts section
- Define AdwShortcutsSection with title "Window"
- Add shortcut items: Minimize window, Fullscreen

**2.4**: Add WhatsApp Web shortcuts section
- Define AdwShortcutsSection with title "WhatsApp Web"
- Add shortcut items: Find in chat, Search chats, New chat, Archive current chat, Open profile, Reload WhatsApp Web

**2.5**: Add WebView Zoom shortcuts section (conditional)
- Define AdwShortcutsSection with ID "webview_zoom_section" and title "WebView Zoom"
- Add shortcut items: Zoom in, Zoom out, Reset zoom
- Include multiple accelerators where applicable (e.g., Ctrl++ and Ctrl+Numpad+)

**2.6**: Add Accessibility shortcuts section (conditional)
- Define AdwShortcutsSection with ID "accessibility_section" and title "Accessibility"
- Add shortcut items: Toggle high contrast, Toggle focus indicators

**2.7**: Add Developer shortcuts section (conditional)
- Define AdwShortcutsSection with ID "developer_section" and title "Developer"
- Add shortcut items: Developer tools, Reload page, Force reload

**2.8**: Add Notifications shortcuts section (conditional)
- Define AdwShortcutsSection with ID "notifications_section" and title "Notifications"
- Add shortcut items: Toggle notifications, Toggle Do Not Disturb

**Validation**:
- [ ] Blueprint file syntax is valid
- [ ] All sections are defined with proper structure
- [ ] All shortcut items have title and accelerator properties
- [ ] Translatable strings use _() syntax
- [ ] Conditional sections have proper IDs for visibility control

**Deliverable**: Complete shortcuts-dialog.blp file with all shortcuts defined

---

### 3. Update Build System for Blueprint Compilation
**Status**: Pending
**Estimated Effort**: 10 minutes
**Parallelizable**: Can start after task 2
**Blocks**: Task 6

Update meson.build to compile shortcuts-dialog.blp:

- Add `data/ui/shortcuts-dialog.blp` to blueprint sources list (or create separate blueprint_compile call)
- Ensure compiled UI is output to correct location
- Add compiled `shortcuts-dialog.ui` to GResource bundle at path `/io/github/tobagin/karere/shortcuts-dialog.ui`
- Test compilation: `ninja -C build`

**Validation**:
- [ ] Blueprint file compiles without errors
- [ ] `shortcuts-dialog.ui` is generated in build directory
- [ ] UI file is included in GResource bundle
- [ ] Resource is accessible at runtime path `/io/github/tobagin/karere/shortcuts-dialog.ui`
- [ ] No build warnings related to Blueprint compilation

**Deliverable**: Updated build configuration that compiles and bundles shortcuts-dialog.ui

---

### 4. Refactor ShortcutsWindow Class - Remove Old Implementation
**Status**: Pending
**Estimated Effort**: 15 minutes
**Parallelizable**: No (requires task 2 completion)
**Blocks**: Task 5

Remove programmatic UI building code from ShortcutsWindow:

- Remove instance variables: `content_page`, `scrolled_window`
- Remove method: `setup_dialog()` (or gut implementation)
- Remove method: `create_shortcuts_content()`
- Remove method: `add_general_shortcuts()`
- Remove method: `add_window_shortcuts()`
- Remove method: `add_whatsapp_shortcuts()`
- Remove method: `add_accessibility_shortcuts()`
- Remove method: `add_developer_shortcuts()`
- Remove method: `add_webview_zoom_shortcuts()`
- Remove method: `add_notification_shortcuts()`
- Remove method: `add_shortcut_row()`
- Remove method: `refresh_shortcuts_content()`

**Validation**:
- [ ] All removed methods are no longer referenced
- [ ] Code compiles without errors
- [ ] No unused imports remain

**Deliverable**: Cleaned ShortcutsWindow class with old implementation removed

---

### 5. Refactor ShortcutsWindow Class - Implement UI Loading
**Status**: Pending
**Estimated Effort**: 30 minutes
**Parallelizable**: Must follow task 4
**Blocks**: Task 6

Implement new UI loading logic in ShortcutsWindow:

**5.1**: Add instance variables for section references
```vala
private Adw.ShortcutsSection? webview_zoom_section = null;
private Adw.ShortcutsSection? accessibility_section = null;
private Adw.ShortcutsSection? developer_section = null;
private Adw.ShortcutsSection? notifications_section = null;
```

**5.2**: Implement constructor UI loading
- Create Gtk.Builder instance
- Load UI from resource: `/io/github/tobagin/karere/shortcuts-dialog.ui`
- Retrieve dialog object with ID "shortcuts_dialog"
- Retrieve section objects for conditional sections
- Handle errors gracefully with try-catch

**5.3**: Implement initial visibility setup
- Create method `update_section_visibility()`
- Check settings and update visibility for each conditional section
- Call from constructor after loading UI

**Validation**:
- [ ] Dialog loads from UI resource successfully
- [ ] All section references are obtained correctly
- [ ] No runtime errors when instantiating ShortcutsWindow
- [ ] Code compiles without errors

**Deliverable**: ShortcutsWindow class that loads AdwShortcutsDialog from UI resource

---

### 6. Implement Dynamic Section Visibility
**Status**: Pending
**Estimated Effort**: 20 minutes
**Parallelizable**: Must follow task 5

Implement settings-based section visibility control:

**6.1**: Update settings listener setup
- Modify `setup_settings_listeners()` to connect to visibility update method
- Keep existing setting listeners: keyboard-shortcuts-enabled, webview-zoom-enabled, accessibility-shortcuts-enabled, developer-shortcuts-enabled, developer-tools-enabled, notification-shortcuts-enabled

**6.2**: Implement visibility update logic
- Implement `update_section_visibility()` method
- WebView Zoom section: visible when `webview-zoom-enabled == true`
- Accessibility section: visible when `accessibility-shortcuts-enabled == true`
- Developer section: visible when `developer-shortcuts-enabled == true AND developer-tools-enabled == true`
- Notifications section: visible when `notification-shortcuts-enabled == true`
- Handle master switch: hide all when `keyboard-shortcuts-enabled == false`

**6.3**: Test visibility updates
- Test with different settings combinations
- Test real-time updates while dialog is open

**Validation**:
- [ ] Sections show/hide based on settings correctly
- [ ] Visibility updates work in real-time
- [ ] All settings combinations work as expected
- [ ] No visual glitches during visibility changes

**Deliverable**: Dynamic section visibility based on settings

---

### 7. Update Project Documentation
**Status**: Pending
**Estimated Effort**: 10 minutes
**Parallelizable**: Can be done any time after task 1

Update openspec/project.md to reflect LibAdwaita 1.8.0 requirement:

- Change LibAdwaita version from ">=1.3.0" to ">=1.8.0" in Tech Stack section
- Add AdwShortcutsDialog to the list of used LibAdwaita components (if there is such a list)
- Update any version constraints documentation

**Validation**:
- [ ] project.md accurately reflects minimum LibAdwaita version
- [ ] All version references are updated
- [ ] Documentation is clear and consistent

**Deliverable**: Updated project.md with correct version requirements

---

### 8. Test Shortcuts Dialog Presentation
**Status**: Pending
**Estimated Effort**: 15 minutes
**Parallelizable**: Must follow task 6

Test all methods of presenting the shortcuts dialog:

**Test Cases**:
1. Press Ctrl+? while main window has focus
2. Press Ctrl+/ while main window has focus
3. Click "View Keyboard Shortcuts" button in Preferences
4. Navigate to Help menu and select "Keyboard Shortcuts" (if applicable)

**Validation**:
- [ ] Dialog appears correctly for all presentation methods
- [ ] Dialog is centered on parent window
- [ ] All shortcuts are displayed correctly
- [ ] Accelerator formatting is readable
- [ ] Dialog can be closed with Escape key
- [ ] Dialog can be closed by clicking outside (if applicable)

**Deliverable**: Verified shortcuts dialog presentation from all entry points

---

### 9. Test Settings Integration
**Status**: Pending
**Estimated Effort**: 20 minutes
**Parallelizable**: Must follow task 6

Test settings-based visibility and interaction:

**Test Cases**:
1. Disable keyboard shortcuts in preferences → verify all sections hidden + message shown
2. Enable keyboard shortcuts → verify always-visible sections shown
3. Toggle webview-zoom-enabled → verify WebView Zoom section visibility
4. Toggle accessibility-shortcuts-enabled → verify Accessibility section visibility
5. Toggle developer-shortcuts-enabled → verify Developer section visibility
6. Toggle developer-tools-enabled while developer-shortcuts-enabled is true → verify Developer section visibility
7. Toggle notification-shortcuts-enabled → verify Notifications section visibility
8. Change settings while dialog is open → verify real-time visibility updates

**Validation**:
- [ ] All settings affect visibility correctly
- [ ] Real-time updates work without closing/reopening dialog
- [ ] No crashes or errors when toggling settings
- [ ] Visual transitions are smooth
- [ ] Disabled message appears when keyboard-shortcuts-enabled is false

**Deliverable**: Verified settings integration and dynamic visibility

---

### 10. Test Accessibility
**Status**: Pending
**Estimated Effort**: 15 minutes
**Parallelizable**: Must follow task 6

Verify accessibility features work correctly:

**Test Cases**:
1. Navigate dialog with keyboard (Tab, Shift+Tab, Arrow keys)
2. Test with screen reader (Orca) if available
3. Verify focus indicators are visible
4. Test high contrast mode rendering
5. Verify all text is readable and properly labeled

**Validation**:
- [ ] Full keyboard navigation works
- [ ] Screen reader announces sections and shortcuts correctly
- [ ] Focus indicators are clear and visible
- [ ] High contrast mode renders correctly
- [ ] No accessibility regressions compared to previous implementation

**Deliverable**: Verified accessibility compliance

---

### 11. Test Build and Runtime on Clean Environment
**Status**: Pending
**Estimated Effort**: 20 minutes
**Parallelizable**: Must follow all implementation tasks

Test in clean build environment:

**Test Steps**:
1. Clean build directory: `rm -rf build`
2. Reconfigure: `meson setup build`
3. Build: `ninja -C build`
4. Install (Flatpak): `flatpak-builder --user --install build-dir io.github.tobagin.karere.Devel.json` (or equivalent)
5. Run application: `flatpak run io.github.tobagin.karere.Devel`
6. Open shortcuts dialog: Ctrl+?
7. Test all functionality

**Validation**:
- [ ] Clean build completes successfully
- [ ] No build warnings or errors
- [ ] Application launches correctly
- [ ] Shortcuts dialog works as expected
- [ ] No runtime warnings or errors in logs

**Deliverable**: Verified clean build and runtime functionality

---

### 12. Update Translation Strings
**Status**: Pending
**Estimated Effort**: 10 minutes
**Parallelizable**: Can start after task 2

Update translation templates for new Blueprint file:

- Regenerate .pot file to include strings from shortcuts-dialog.blp
- Verify all translatable strings are marked with _()
- Check that string contexts are preserved
- Update existing translations if needed (or mark for retranslation)

**Validation**:
- [ ] .pot file includes all shortcuts dialog strings
- [ ] String extraction is complete
- [ ] No duplicate or missing translation contexts
- [ ] Existing translations are preserved where applicable

**Deliverable**: Updated translation templates

---

### 13. Final Integration Testing
**Status**: Pending
**Estimated Effort**: 30 minutes
**Parallelizable**: Must be last task

Comprehensive integration testing:

**Test Matrix**:
- Test all shortcuts in all sections
- Test with all combinations of enabled/disabled settings
- Test dialog presentation from all entry points
- Test real-time settings updates
- Test keyboard navigation
- Test with different window sizes
- Test with different themes (light/dark)
- Test with high contrast mode
- Test accessibility features

**Regression Testing**:
- Verify no existing functionality is broken
- Compare behavior with previous implementation
- Check for any visual or functional differences

**Validation**:
- [ ] All test cases pass
- [ ] No regressions identified
- [ ] User experience is equivalent or better
- [ ] Performance is acceptable
- [ ] No memory leaks or crashes

**Deliverable**: Fully tested and validated implementation

---

## Summary

**Total Tasks**: 13
**Critical Path**: Tasks 1 → 2 → 3 → 4 → 5 → 6 → 8 → 9 → 13
**Parallel Opportunities**: Tasks 7 and 12 can be done concurrently with implementation tasks
**Estimated Total Effort**: ~4 hours

## Risk Mitigation

**High Risk Areas**:
1. Blueprint syntax errors → Mitigate with early compilation testing (task 3)
2. Runtime resource loading failures → Mitigate with error handling in task 5
3. Settings integration complexity → Mitigate with thorough testing in task 9

**Rollback Points**:
- After task 1: Can revert dependency change
- After task 6: Can revert to old implementation if major issues found
- After task 13: Final decision point before merging

## Success Metrics

- [ ] Code reduction: ~150+ lines removed from ShortcutsWindow class
- [ ] All shortcuts display correctly in new dialog
- [ ] Dynamic visibility works as expected
- [ ] No regressions in functionality or accessibility
- [ ] Clean build with no warnings
- [ ] All tests pass

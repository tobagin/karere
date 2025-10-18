# Proposal: Migrate to AdwShortcutsDialog

## Why
The current shortcuts window uses a custom implementation with manual UI building code (~270 lines). LibAdwaita 1.8 introduced `AdwShortcutsDialog`, the official widget for displaying keyboard shortcuts in GNOME applications, which provides better integration, reduced code, and a standard user experience. Since the app is distributed via Flatpak with GNOME Platform 49 (which includes LibAdwaita 1.8), we can adopt this new widget to simplify implementation and align with GNOME ecosystem standards.

## What Changes
- Update minimum LibAdwaita dependency from 1.3.0 to 1.8.0 in meson.build
- Create `data/ui/shortcuts-dialog.blp` Blueprint file defining AdwShortcutsDialog structure with all shortcut sections
- Refactor `ShortcutsWindow` class to load dialog from UI resource instead of building programmatically
- Remove ~200+ lines of manual UI construction code (methods for building PreferencesPage, PreferencesGroup, ActionRow components)
- Implement section visibility control using widget properties instead of content rebuilding
- Update openspec/project.md to document LibAdwaita 1.8.0 requirement
- Add Blueprint compilation and resource bundling to build system

## Impact
- **Affected specs**: UI Components (new shortcuts dialog), Keyboard Shortcuts (integration updates)
- **Affected code**:
  - `src/shortcuts-window.vala` - Major refactor (simplified from ~270 to ~100 lines)
  - `src/keyboard-shortcuts.vala` - No changes (maintains same interface)
  - `meson.build` - Dependency version update, Blueprint compilation
  - `data/ui/shortcuts-dialog.blp` - New file
  - `openspec/project.md` - Version documentation update
- **Breaking changes**: None (API compatibility maintained, Flatpak runtime provides LibAdwaita 1.8)
- **Benefits**:
  - Reduced maintenance burden (less custom code)
  - Standard GNOME shortcuts dialog UX
  - Better accessibility (inherited from AdwShortcutsDialog)
  - Declarative UI structure (easier to modify)
  - Future-proof (official widget receives updates)

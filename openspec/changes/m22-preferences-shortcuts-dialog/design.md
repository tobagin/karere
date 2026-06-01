## Context

Karere v3 shipped a hand-rolled `AdwPreferencesWindow` in Rust (`src/preferences.rs`, 611 lines) that built every row imperatively. v4 has accumulated all the underlying GSettings keys across M7–M21 but no UI to expose them, leaving `gsettings set` as the only configuration path. Independently, M8 declared an `app.show-help-overlay` action and bound Ctrl+? to it but left the handler stubbed because no shortcuts dialog yet existed. M22 closes both gaps with one composite UI delivery using blueprint files compiled into the gresource bundle.

## Goals / Non-Goals

**Goals:**
- Surface every GSetting introduced in M7–M21 through a single `AdwPreferencesDialog` with the six-section shape karere v3 used, updated for v4 semantics where keys changed meaning.
- Provide an `AdwShortcutsDialog` listing every accelerator the shell binds, presented via the existing `app.show-help-overlay` action.
- Use blueprint (`.blp`) composite templates rather than imperative widget construction to keep row-level changes diff-friendly.
- Present both dialogs as modal `AdwDialog`s parented to the active window (consistent with libadwaita 1.5 guidance), not as top-level windows.

**Non-Goals:**
- Introducing any new GSettings keys; M22 only surfaces existing ones.
- Per-account permission editing (deferred to M20's account dialog).
- Migrating existing imperative dialogs (e.g., M5 permission prompt) to blueprint.
- Localization beyond the language-detect dropdown selection (translation strings remain in PO files maintained outside this change).

## Decisions

- **Blueprint composite templates over imperative builders**: declarative `.blp` files keep page structure reviewable and let row additions land as small diffs. The Rust side is a thin subclass that binds settings to widgets named in the template.
- **`AdwPreferencesDialog` not `AdwPreferencesWindow`**: libadwaita 1.5+ deprecates the window variant; v4 uses the dialog variant for both mobile and desktop responsiveness (also aligns with M21).
- **Setting → widget binding via `gio::Settings::bind`**: every switch, dropdown, and slider uses `settings.bind("key", &widget, "property", SettingsBindFlags::DEFAULT)` so no manual signal plumbing per row.
- **Permission list as `GtkListBox` populated from M11 registry**: rebuilt on dialog open (no live subscription), with a per-row delete button calling the M11 remove API and a footer Clear-all calling the M11 clear API. Cheap because the registry is small.
- **Spellcheck language list with star-pin**: a `GtkListBox` row per available Chromium language; clicking the star sets a sort key in the existing M16 pinned-languages GSetting (a string array of locale codes). Pinned codes render at the top of the list and at the top of the language dropdown on next open.
- **Download-directory picker via `gtk::FileDialog::select_folder`**: invoked by a row-action button; on completion writes the path to the `download-directory` GSetting which M12's download handler reads.
- **Shortcuts dialog as a sibling blueprint**: `AdwShortcutsDialog` accepts groups of `AdwShortcutsItem` rows; one group per category (Navigation, Editing, View, System) keeps karere v3's mental model.
- **Action handler ownership in `application.rs`**: both `app.preferences` and `app.show-help-overlay` resolve to handlers that look up the active window and call `dialog.present(Some(&window))`. The dialog types own no application state beyond the `gio::Settings` reference.
- **Notice rows for restart-required and Chromium-native semantics**: `AdwActionRow` with subtitle text rather than a separate banner widget — keeps the section visually unified.

## Risks / Trade-offs

- Blueprint compilation adds a build-time dependency (`blueprint-compiler`); already present for prior `.blp` files in the project, so no new burden.
- Star-pin UI for spellcheck languages depends on M16 exposing a stable pinned-list GSetting; if M16 instead used a different storage shape (e.g., GVariant dictionary), the row needs to adapt — verify M16's chosen schema before binding.
- `download-directory` is a free-form path; if the user picks a non-writable directory the failure surfaces only at download time. We accept this rather than pre-validating, matching karere v3 behavior.
- The Permissions page is a snapshot, not live: if another window grants a permission while the dialog is open, the list goes stale until reopened. Acceptable because permission grants are rare and the dialog is short-lived.
- `app.show-help-overlay` historically referred to `GtkShortcutsWindow`; we deliberately route it to `AdwShortcutsDialog` instead. The action name is kept for backward compatibility with the Ctrl+? accelerator binding from M8.
- About footer needs the build-time version string; we read it from `env!("CARGO_PKG_VERSION")` rather than a separate manifest file to avoid drift.

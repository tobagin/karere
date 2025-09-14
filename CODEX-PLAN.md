# Karere – CODEX Improvement Plan

This plan outlines actionable refactors and improvements for maintainability, testability, and long‑term evolution of the Karere GTK4/LibAdwaita app.

## Snapshot
- Stack: GTK4, LibAdwaita, WebKitGTK 6.0, Libsoup 3, GLib/GIO, Vala.
- UI: Blueprint compiled to `.ui` via Meson; resources bundled.
- Notable modules:
  - App lifecycle: `src/main.vala`, `src/application.vala`
  - Window + Web: `src/window.vala` (WhatsApp Web integration)
  - Preferences: `src/preferences.vala`
  - WebKit config: `src/webkit-manager.vala`
  - Notifications: `src/notification-manager.vala`
  - Accessibility: `src/accessibility-manager.vala`
  - Shortcuts: `src/keyboard-shortcuts.vala`, `src/shortcuts-window.vala`
  - Crash reporting: `src/crash-reporter.vala`
  - Misc: `src/logger.vala`, `src/utils.vala`, `src/about-dialog.vala`

Largest files by line count (prime refactor targets):
- src/window.vala (~1073)
- src/preferences.vala (~695)
- src/crash-reporter.vala (~660)
- src/keyboard-shortcuts.vala (~528)
- src/application.vala (~481)
- src/accessibility-manager.vala (~451)

## Key Opportunities

1) Split Monoliths into Cohesive Components
- Window (src/window.vala:1)
  - Create a `window/` package with focused controllers:
    - WebViewController: creation, load, errors, user agent injection.
    - PermissionController: notification/permission prompts and persistence.
    - DragAndDropController: DnD hookup and share flows.
    - WindowStateManager: persist/restore size and maximized state.
    - Toasts: small helper for info/error/success messages.
    - AccessibilityOverlay: ARIA roles, focus management.
  - Window becomes a thin composition root wiring these controllers.

- Preferences (src/preferences.vala:1)
  - Split into `preferences/` subpages mapped to `Adw.PreferencesPage`:
    - GeneralPage, AccessibilityPage, ZoomPage, NotificationsPage, DndPage, SpellCheckPage, CrashReportingPage, LoggingPage.
  - Each page owns its binds, visibility toggles, and actions.

- Crash Reporter (src/crash-reporter.vala:1)
  - Extract components:
    - CrashSignalHandler: install/restore signal handlers.
    - CrashReportBuilder: capture metadata, system info, stack traces.
    - CrashReportStore: paths and persistence.
    - CrashSubmissionClient: async HTTP submission via libsoup.
    - CrashDialogs: Alert/Details/Result dialogs.
  - Inject Logger + Settings instead of constructing internally.

- Keyboard Shortcuts (src/keyboard-shortcuts.vala:1)
  - Split by concern:
    - AppAccels, WindowAccels, WebViewAccels, AccessibilityAccels, DeveloperAccels, NotificationAccels, WhatsAppAccels.
  - Add a central ShortcutRegistry (see “Single Source of Truth”) consumed by both implementation and the help UI.

2) Single Source of Truth (reduce duplication and drift)
- SettingsService
  - Provide a typed wrapper around `GLib.Settings` with constants/enums for keys and helper signals.
  - Construct once and inject where needed (Application creates it; others receive it), replacing repeated `new Settings(Config.APP_ID)` calls.

- LoggerProvider
  - Create one Logger instance at app startup; pass it down (constructor injection) to avoid ad‑hoc `new Logger()` in multiple classes (e.g., src/about-dialog.vala:1, src/shortcuts-window.vala:1, src/webkit-manager.vala:1, src/window.vala:1).

- ShortcutRegistry
  - Define a canonical list of actions → accelerators + labels.
  - KeyboardShortcuts applies accels; ShortcutsWindow renders them. Eliminates divergence between implementation and help content.

- Constants Module
  - Centralize constants: URLs (WhatsApp Web, website, issue/support links), CSS class names (e.g., `karere-*`), icons, default user agent string, file paths.

3) API Modernization and Deprecations
- Shortcuts API
  - `KeyboardShortcuts.get_shortcuts_sections()` references `Gtk.ShortcutsSection` with a note about GTK4 deprecation. Remove this method and rely entirely on the custom `ShortcutsWindow` based on Adw/Preferences content.

- About Dialog automation
  - `KarereAboutDialog` simulates tab/enter navigation to open release notes (src/about-dialog.vala:202, :236, :247). This is brittle. Replace with:
    - A dedicated “What’s New” flow using `Adw.AlertDialog` that reads the same release notes content directly, or
    - An explicit button/link from about dialog to present the release notes dialog, removing focus simulation entirely.

- Release notes parsing
  - Current approach uses Regex on XML and HTML. Prefer a real parser to robustly extract the `<release>` matching `Config.VERSION`:
    - Option A: add `libxml-2.0` and use `Xml` bindings.
    - Option B: keep markup intact and set release_notes directly in `Adw.AboutDialog` if compliant. Avoid HTML→text munging; keep as markup.

- WebKit 6
  - Cookie/Website data APIs changed; current code disables manual setup and falls back to defaults. Introduce a small `WebContextFactory` that configures a persistent context safely for 6.0 without using removed APIs, or explicitly document why defaults are sufficient.

- Blueprint compile portability
  - Meson passes a fixed `--typelib-path` to the blueprint-compiler. Consider dropping or making it conditional to avoid distro‑specific paths.

4) Cross‑cutting Quality
- Error/Alert Dialogs
  - Reuse a small DialogService for common Alert/confirm patterns; wire callbacks; consistently parent to the main window.

- Environment/log configuration
  - `G_MESSAGES_DEBUG` / `SOUP_DEBUG` logic appears in both `src/main.vala:1` and `src/application.vala:200+`. Consolidate into one function and call at deterministic points (pre‑GTK in main, re‑apply user prefs in startup) using shared helpers.

- User agent override
  - Prefer WebKit settings to set UA; keep JS override as a guarded fallback (behind a setting or only when detection fails). Log clearly when fallback is used.

- Accessibility CSS
  - Centralize CSS class names and class toggling helpers to reduce repetition across Window and AccessibilityManager.

## Concrete Refactors (Proposed File Map)
- src/services/
  - settings-service.vala
  - logger-provider.vala
  - dialog-service.vala
  - constants.vala
- src/window/
  - window.vala (thin composition root; current class trimmed)
  - webview-controller.vala
  - permission-controller.vala
  - dnd-controller.vala
  - window-state.vala
  - toasts.vala
  - accessibility-overlay.vala
- src/preferences/
  - preferences.vala (container dialog; wires pages)
  - page-general.vala
  - page-accessibility.vala
  - page-zoom.vala
  - page-notifications.vala
  - page-dnd.vala
  - page-spellcheck.vala
  - page-crashreport.vala
  - page-logging.vala
- src/crash/
  - crash-reporter.vala (orchestrator)
  - crash-signal-handler.vala
  - crash-report-builder.vala
  - crash-store.vala
  - crash-submit-client.vala
  - crash-dialogs.vala
- src/shortcuts/
  - registry.vala
  - keyboard-shortcuts.vala (split helpers)
  - shortcuts-window.vala (render from registry)
- src/webkit/
  - webkit-manager.vala (use injected services)
  - web-context-factory.vala
- src/about/
  - about-dialog.vala (remove focus simulation)
  - release-notes.vala (parsing/formatting)

## Testing Implications
- Adjust unit tests to target new modules:
  - Window: test WebViewController, state persistence, permission flows independently.
  - Preferences: test each page’s binds/visibility logic in isolation.
  - Crash: test CrashReportBuilder and CrashSubmissionClient with fixtures (no network).
  - Shortcuts: test ShortcutRegistry (definitions) and that both binding and rendering consume the same data.
- Keep existing test names and suites where possible, update imports/paths.

## Sequenced Work Plan
1. Foundations (services + constants)
- Add SettingsService and LoggerProvider; migrate modules to injection.
- Introduce DialogService and Constants (CSS, URLs, icons, UA).

2. Keyboard shortcuts as a single source of truth
- Add ShortcutRegistry; migrate KeyboardShortcuts to consume it.
- Update ShortcutsWindow to render from registry; remove deprecated `get_shortcuts_sections()`.

3. Window split
- Extract WebViewController, WindowState, DnD, Toasts, AccessibilityOverlay, PermissionController.
- Keep `Karere.Window` as thin composition root.

4. Preferences split
- Create per‑page classes; move binds/visibility logic out of the monolith.

5. Crash reporter split
- Separate signal handler, builder, store, submission, and dialogs; make submission fully async and resilient.

6. About dialog + release notes
- Add ReleaseNotes helper; remove focus simulation from `about-dialog.vala`.

7. WebKit context modernization
- Add WebContextFactory and document 6.0 choices; keep API drift isolated.

8. Meson polish
- Update sources lists; make blueprint `--typelib-path` conditional or remove if unnecessary.

9. Cleanup & docs
- Remove dead code; update README and docs on architecture, testing, and contribution patterns.

## Quick Wins (Low‑risk, High‑value)
- Remove `KeyboardShortcuts.get_shortcuts_sections()` entirely and migrate ShortcutsWindow to registry.
- Replace manual XML/HTML Regex in about dialog with ReleaseNotes helper; avoid focus simulation.
- Centralize UA setting and gate JS UA override behind a preference.
- Introduce Constants for CSS and URLs; replace literals.

## Risks & Mitigations
- File splits can affect Meson/test wiring → Tackle in small PRs (module by module) and keep commit scope minimal.
- Dependency updates (e.g., libxml2 for XML parsing) → Make optional or guarded, document in meson options.
- Behavioral drift in keyboard shortcuts → Registry must encode both labels and accels and be covered by tests.

## Notes on Specific References
- Heavy files to split:
  - src/window.vala:1
  - src/preferences.vala:1
  - src/crash-reporter.vala:1
  - src/keyboard-shortcuts.vala:1
- Deprecated comment and redundant method:
  - src/keyboard-shortcuts.vala:503
- Focus simulation to remove:
  - src/about-dialog.vala:202
  - src/about-dialog.vala:236
  - src/about-dialog.vala:247
- Settings duplication to centralize:
  - Multiple occurrences of `new Settings(Config.APP_ID)` across `src/*`.
- Logger duplication to centralize:
  - New Logger instances in about-dialog, shortcuts-window, webkit-manager, window.

---
This plan favors small, verifiable steps. I can start with SettingsService + LoggerProvider and the shortcuts registry to unlock the rest with minimal churn. 

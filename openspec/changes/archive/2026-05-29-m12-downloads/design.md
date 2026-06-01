## Context

CEF emits download events through a `CefDownloadHandler` whose `on_before_download` callback chooses the target path and whether to show the native chooser, and whose `on_download_updated` callback streams progress / completion / cancellation. Karere v3 (`window.rs:1142-1296`) provides the reference behaviour: target directory taken from a GSetting, sanitized suggested filename, `(N)` suffix collision resolution, and a completion toast with "Open" and "Show in Folder" actions. v4 must reproduce that flow while integrating with v4's `wrap_download_handler!` macro, the existing shared-state event bus, and the tokio runtime that hosts portal calls.

## Goals / Non-Goals

**Goals:**
- Deliver WhatsApp Web media downloads onto disk with no overwrite of prior files of the same name.
- Honour a user-configurable download directory, defaulting to `XDG_DOWNLOAD_DIR`.
- Provide a one-step toast UX: "Open" launches the file in the default app via OpenURI; "Show in Folder" opens its parent.
- Surface failures (cancelled / error code) via `AdwAlertDialog`.

**Non-Goals:**
- Progress UI per-download (a single "downloading" toast on start plus completion toast is sufficient).
- Pause / resume / cancel controls.
- Aggregated multi-file batch toasts.
- Preferences UI for the new gschema keys (the row lives in M22).

## Decisions

**1. Target directory resolution lives in the handler, not the action layer.**
The handler reads `download-directory` directly from `gio::Settings` on each download. Rationale: keeps the action layer (`app.open-download`) free of CEF concerns; matches v3's locality. Alternative considered: caching on `ShellClient` startup — rejected because users can change the setting mid-session and we want it to take effect immediately.

**2. Collision resolution uses ` (N)` suffix walking, not timestamp suffixes.**
Match Karere v3 exactly: `name.ext`, `name (1).ext`, `name (2).ext`, … This is what users already expect from GNOME Files. Alternative considered: a single timestamp suffix `name-2026-05-27-17-08-00.ext` — rejected because it produces ugly filenames and breaks v3 parity.

**3. Show CEF's own save dialog?** No. `callback.cont(target_path, false)` — we pick the path silently. Rationale: WhatsApp media is small and frequent; a dialog per save is friction. Power users who want to choose paths can change the GSetting (and M22 will expose a "Always ask" option if needed; out of scope here).

**4. Completion is signalled through shared state, not direct GTK calls.**
The download handler runs on a CEF thread; it cannot touch GTK widgets. It pushes a `DownloadCompleted { path, name }` (or `DownloadFailed { name, reason }`) into the existing `SharedState` event queue; the window's polling tick drains the queue on the main thread and raises the toast / dialog. Alternative considered: `glib::MainContext::default().spawn_local(...)` directly from the handler — rejected because the handler doesn't have a `MainContext` reference and we already have a working state pump from M08.

**5. `app.open-download <path>` uses `ashpd::desktop::open_uri::OpenURI`.**
This goes through the FreeDesktop portal so it works under Flatpak without filesystem holes. The call must run on the tokio runtime because `ashpd` is async; the action handler spawns onto the existing runtime started in M01.

**6. "Show in Folder" uses the same OpenURI portal on the parent directory.**
GNOME Files registers as a handler for `inode/directory`, so `open_file` on the parent path works. Alternative considered: `OpenURI.open_directory` — equivalent in practice; pick `open_file(parent)` for symmetry with "Open".

**7. Filename sanitization strips path separators and NUL only.**
Anything more aggressive (e.g. stripping all non-ASCII) breaks Unicode filenames legitimately produced by WhatsApp. v3 also took this minimal approach.

**8. New gschema keys land now even though M22 owns the UI.**
Schema changes need to ship in the same release as the consuming code, and gschema migrations are awkward to defer. M22 will only add `AdwActionRow` widgets bound to existing keys.

## Risks / Trade-offs

- [Risk] The `download-directory` GSetting could point to a non-existent or unwritable directory → Mitigation: when `create_dir_all` fails, fall back to `XDG_DOWNLOAD_DIR` and log a warning; surface the warning as a toast on next download attempt.
- [Risk] Filename collision walk could loop indefinitely on a pathological directory (e.g. millions of `(N)` files) → Mitigation: cap the walk at 9999 iterations; on exhaustion, append a UUID suffix and continue.
- [Risk] Portal `open_file` returns an error inside Flatpak when the file is on a non-exported path → Mitigation: ensure target directory is inside the user's home; the default (`XDG_DOWNLOAD_DIR`) already is. Document the constraint in M22 prefs.
- [Trade-off] No progress UI means large downloads (videos) give no feedback between start and completion. Acceptable for v4.0 because the typical WhatsApp file is < 16 MB.
- [Risk] Race condition: two downloads of the same filename starting near-simultaneously could both resolve to `name (1).ext` before either creates a file → Mitigation: in collision resolution, attempt to atomically `O_CREAT | O_EXCL` a sentinel; on `EEXIST`, increment and retry.

## Migration Plan

- New gschema keys are additive with safe defaults (`""`, `true`, `toast`), so existing installs upgrade without intervention.
- No rollback required beyond reverting the binary; gschema keys can be left in place harmlessly.

## Open Questions

- Should `notify-download-type = notification` emit a desktop notification via `gio::Notification` instead of (or in addition to) the toast? Leaving the enum value defined now; the `notification` and `both` branches can stub to "toast only" until a follow-up. Decision deferred to M22 / a later notifications milestone.

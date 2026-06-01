## Why

Karere v3 routes non-WhatsApp links to the default browser (`window.rs:2047-2081`), recovers from web-process crashes (`window.rs:2083-2127`), strips "Open in new window/tab/incognito" entries from the link context menu (`window.rs:2130-2146`), and retries load failures on autostart with exponential backoff (`window.rs:2148-2188`). The CEF shell currently lets external links navigate the embedded view, dies on renderer crash, exposes WebKit/Chromium-default "Open in New Window" items that have no destination, and gives up after a single transient load failure. Milestone M09 ports each of those four behaviors onto CEF so the shell is functionally on par with Karere v3's browsing surface before we move on to find-in-page (M10) and permission persistence (M11).

## What Changes

- Add a CEF `RequestHandler` that intercepts top-level navigations and launches non-WhatsApp URIs via `gio::AppInfo::launch_default_for_uri`, allowing only `whatsapp.com`, `whatsapp.net`, `web.whatsapp.com`, and the inert schemes `data:`, `blob:`, `about:`, `file:`, `chrome-error:`.
- In that same `RequestHandler`, react to `on_render_process_terminated` by publishing a `crash_toast` on `SharedState`, reloading the browser after a 1500 ms backoff for up to 5 crashes per 60 s window, and surfacing an `AdwAlertDialog` with an "Open logs" action once that threshold is exceeded.
- Add a CEF `ContextMenuHandler` that walks the supplied menu model and removes `MENU_ID_OPEN_LINK_NEW_WINDOW`, `MENU_ID_OPEN_LINK_NEW_TAB`, and `MENU_ID_OPEN_LINK_IN_INCOGNITO_WINDOW`, plus the separators that immediately wrap those entries.
- Extend the existing `LoadHandler` with `on_load_error`: ignore `ERR_ABORTED`, increment `SharedState.load_error_count`, schedule a reload via `glib::timeout_add_local` with `delay_ms = min(60_000, 500 * 2.pow(count))`, and reset the counter inside `on_load_end`.
- Wire the new handlers into `ShellClient` and surface `crash_toast` through the 100 ms polling loop introduced in M8 so toasts appear via the M8 toast overlay.

## Capabilities

### New Capabilities
- `cef-request-handler`: route external links to the host browser and recover gracefully from renderer crashes.
- `cef-context-menu`: provide a CEF `ContextMenuHandler` that strips "open in new window/tab/incognito" entries so the shell exposes only navigation actions it can honor.
- `cef-load-handler-extended`: extend the existing `LoadHandler` with `on_load_error` exponential-backoff retries that respect `ERR_ABORTED` and reset on successful load completion.

### Modified Capabilities
<!-- None: M09 introduces new handlers; existing capabilities keep their requirements. -->

## Impact

- New files: `src/handlers/request.rs`, `src/handlers/context_menu.rs`.
- Modified files: `src/handlers/load.rs` (adds `on_load_error`/`on_load_end` retry, offline flag, main-frame gating, error-page handling), `src/handlers/life_span.rs` (adds `on_before_popup` popup routing), `src/handlers/client.rs` (registers the new handlers), `src/handlers/mod.rs` (`SharedState` gains `crash_toast`, `crash_history`, `crash_dialog_request`, `pending_reload`, `load_error_count`, `offline`), `src/window.rs` (new 100 ms poll loop drains crash toast/dialog and the offline overlay).
- Ancillary changes discovered during verification (no dedicated capability): `cef::sys::cef_menu_id_t` lacks `MENU_ID_OPEN_LINK_*` in cef-rs 148, so the link-open command ids are pinned by value (50100/50101/50102); `--no-zygote` is now the default so renderers are standalone `--type=renderer` processes (single-webview app gains little from the zygote and crash recovery becomes testable); `main.rs` gains `--debug` / `--debuglevel=LEVEL` flags (default INFO) controlling the `karere` log level.
- Dependencies: relies on the existing `cef`/`cef-rs 148` bindings, `gtk4`, `libadwaita`, `gio`, and `glib`; no new crates.
- Behavioral impact: external links no longer hijack the embedded view; renderer crashes self-heal with user-visible feedback; "Open in New Window/Tab/Incognito" disappears from the link menu; initial-load network blips no longer leave a blank page.

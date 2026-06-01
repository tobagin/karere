## Why

Developer ergonomics are missing: the shell has no way to open Chromium DevTools, and Karere v3's `Ctrl+F` was a passthrough that surfaced WhatsApp Web's own in-page search rather than Chromium's native find. We want F12 to open real Chromium DevTools embedded in the window (docked below the page) and Ctrl+F to drive `BrowserHost::find` so users get an actual cross-page match counter and Next/Prev cycling, independent of whatever the embedded site implements.

## What Changes

- Add `win.show-devtools` action (accels `F12`, `<Primary><Shift>i`) that toggles embedded DevTools: load the active page's CDP DevTools frontend (discovered via `--remote-debugging-port`) into an OSR `KarereWebView` docked in a vertical `gtk::Paned`. (`ShowDevTools` can't be used — CEF 148 refuses windowless DevTools.)
- Add `win.close-devtools` action + pane close button that close the DevTools view and collapse the pane.
- Enable `--remote-debugging-port`, `--remote-allow-origins=*`, and disable PNA/LNA blocking so the public DevTools frontend can reach the loopback CDP endpoint.
- Add `src/handlers/find.rs` implementing `FindHandler::on_find_result`, wiring `count` and `active_match_ordinal` into `SharedState` so the UI can render "n of m".
- Register the find handler on `KarereClient` (new `find_handler` field).
- Extend `data/ui/window.blp` with a `gtk::SearchBar` (search entry + Prev/Next + counter label) above the WebView area.
- Add `win.find-in-page` action (accel `<Primary>f`) that reveals the search bar and grabs focus; `Escape` hides it and calls `host.stop_finding(true)`.
- Drive `host.find(text, forward, match_case=false, find_next)` from `connect_search_changed` and the Prev/Next buttons.

## Capabilities

### New Capabilities
- `cef-devtools`: Toggle embedded Chromium DevTools (CDP frontend over OSR) docked inside the shell window via host actions and keyboard accels.
- `cef-find-in-page`: In-page text search driven by `BrowserHost::find` + `FindHandler`, surfaced through a GTK search bar with match counter and navigation.

### Modified Capabilities
<!-- None: DevTools and Find-in-page are net-new capabilities for the shell. -->

## Impact

- New files: `src/handlers/find.rs`, `src/devtools.rs` (CDP target discovery).
- Modified files: `src/handlers/client.rs` (`find_handler`, permissive `build_devtools_for`), `src/handlers/request.rs` (permissive mode), `src/handlers/mod.rs` (export + `FindResult`), `src/web_view.rs` (DevTools view mode, scroll-position fix, hard reload), `src/window.rs` (actions, DevTools pane, search-bar wiring, refresh), `src/cef_runtime.rs` (remote-debugging + LNA/origins flags), `src/application.rs` (accels), `src/main.rs` (module), `data/ui/window.blp` (DevTools `gtk::Paned` + pane header + search bar).
- No new crate dependencies; CDP discovery uses `std::net` and a hand-parsed JSON, and DevTools reuses the existing OSR render/life-span pipeline.
- DevTools is a second OSR browser loading the CDP frontend; no separate native window and no Wayland parenting concern.

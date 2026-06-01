## Context

The shell embeds CEF in off-screen rendering (OSR) mode inside a `GtkGLArea`. Two developer-facing features are still missing compared to Karere v3 and parity expectations:

1. **DevTools**: `CefBrowserHost::ShowDevTools` cannot be used for an embedded view — CEF 148's Chrome runtime hard-refuses windowless rendering for the DevTools browser (`chrome_browser_delegate`: "Windowless rendering is not supported for this DevTools window") and always opens a native top-level window. CEF upstream also no longer bundles a local DevTools frontend. So we embed DevTools the way `chrome://inspect` does: enable `--remote-debugging-port`, discover the active page's CDP target, and load its DevTools frontend page into an ordinary OSR `KarereWebView` docked in a bottom split. The frontend is the remote `chrome-devtools-frontend.appspot.com` build (the only one available), connected over a WebSocket to the loopback CDP endpoint.
2. **Find-in-page**: Karere v3 bound `Ctrl+F` to forward the keystroke to the page, which triggered WhatsApp's own chat search. That is not actual find-in-page; it does not work on other sites and offers no match counter. CEF exposes real find via `BrowserHost::find` + `FindHandler::on_find_result`.

`SharedState` already mediates between CEF callback threads and the GTK main loop via a polling tick; the find-result counter will plug into the same pattern.

## Goals / Non-Goals

**Goals:**
- F12 (and `Ctrl+Shift+I`) toggle an embedded DevTools view (CDP frontend over OSR) docked in the bottom of the window, inspecting the active page.
- `Ctrl+F` reveals a GTK search bar that drives real Chromium find; the bar shows `active of count` and supports Next/Prev; `Escape` closes and stops finding.
- Find handler runs without blocking the CEF UI/IO threads; results reach GTK via shared state.
- No regressions to existing OSR rendering, input forwarding, or shutdown.

**Non-Goals:**
- DevTools-Protocol RPC or any programmatic inspection (deferred to M16).
- Live spellcheck patching via DevTools protocol (M16).
- Detachable / pop-out DevTools or right/left dock positions (bottom dock only for v1).
- Per-site find UI customization beyond the standard match counter.
- Regex or whole-word find toggles (match-case is also off by default for v1).

## Decisions

- **DevTools host = CDP frontend in an OSR view.** A second `KarereWebView::new_devtools()` is a normal OSR browser that loads the DevTools frontend URL — not a `ShowDevTools` browser. It uses a *permissive* client (`ClientBuilder::build_devtools_for`) whose request handler keeps every navigation in-view, so the frontend page is not routed to the external browser like other non-WhatsApp URLs.
- **Target discovery off the main thread.** `crate::devtools::fetch_frontend_url` does a blocking HTTP GET of `127.0.0.1:PORT/json/list` on a worker thread; a short GTK poll loads the resolved URL (or toasts + collapses the pane on failure). The CDP JSON is pretty-printed, so targets are split by brace depth, not a literal `},{`.
- **Target selection prefers the WhatsApp page** and skips `about:blank`, workers, and any DevTools frontend page lingering in the list (which would make DevTools inspect itself).
- **Loopback access.** The frontend is a public origin opening a WebSocket to loopback; Private/Local Network Access blocking is disabled via `--disable-features` (the gate was renamed PNA→LNA), plus `--remote-allow-origins=*`. Without this the frontend loads but stays blank.
- **Toggle semantics.** `win.show-devtools` opens if closed, closes if open; `win.close-devtools` and the pane's close button always close. Closing closes the DevTools browser, removes the OSR view, hides the pane, and forces the main view to repaint (otherwise the freed second `GtkGLArea` leaves it blank).
- **Bottom dock via `gtk::Paned`.** A vertical `gtk::Paned` holds the main view (top) and a `devtools_container` (bottom, with a header label + close button, hidden when closed). The divider is user-draggable; the pane opens at ~60% page / 40% DevTools.
- **Find handler is one struct per `KarereClient`, shared across browsers.** Identifier from `on_find_result` is currently ignored (single active browser per window); we record it but key state by the active browser id so multi-browser support stays open.
- **`FindResult { count: i32, active: i32 }` lives in `SharedState`** behind the same lock as other CEF→GTK signals. The GTK polling tick reads it and updates a `gtk::Label`.
- **Search bar lives in `window.blp`** as a `gtk::SearchBar` directly under the headerbar, above the WebView container. This keeps focus/escape semantics standard.
- **`connect_search_changed` triggers a fresh search** with `find_next=false`; Next/Prev reuse the last query with `find_next=true` so Chromium cycles the existing match set instead of restarting.
- **`Escape` calls `host.stop_finding(clear_selection=true)`** to drop highlights when the bar closes.
- **No Wayland parenting caveat.** Embedding DevTools inside the existing window removes the native-window parenting problem entirely; there is no separate top-level to parent.
- **Remote frontend + screencast.** Since CEF ships no local frontend, the appspot remote frontend is used; it opens with a device/screencast panel. There is no bundled local frontend to default that off, but the preference persists in the (stable) frontend origin's storage once the user closes it.

## Risks / Trade-offs

- **Second OSR pipeline cost.** The DevTools view is a second OSR surface painting while open. Mitigation: it only exists while DevTools is open and is torn down on close; same proven code path as the main view. Tearing down its `GtkGLArea` left the main view blank, so close forces a main-view repaint.
- **Open CDP port.** `--remote-debugging-port` exposes a loopback CDP endpoint for the process lifetime, and LNA blocking is disabled so the frontend can reach it. Loopback-only, but any local process could drive the browser via CDP — acceptable for a dev tool; could be gated behind a setting later.
- **Polling latency for find counter.** The existing SharedState tick interval bounds how quickly the "n of m" label refreshes. Acceptable for typing-speed find; revisit only if users perceive lag.
- **Match-case off by default.** Some users expect case-sensitive find. We accept the simpler v1 surface; toggles can be added later without spec changes if hidden behind the same `host.find` call.
- **DevTools view lifecycle.** Closing closes the DevTools browser, removes the OSR view, and hides the pane; re-opening builds a fresh view and re-resolves the target. The view's life-span handler tracks closure; the window close path tears DevTools down before the main CEF-close gate.
- **FindHandler thread safety.** Implemented via the existing `wrap_*_handler!` macros that the project already uses for other handlers, so concurrency is handled the same way as elsewhere.

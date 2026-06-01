## 1. DevTools embedded via CDP frontend

- [x] 1.1 Add `win.show-devtools` GAction (accels `F12`, `<Primary><Shift>i`) that toggles DevTools open/closed; add `win.close-devtools` + pane close button that always close.
- [x] 1.2 Enable `--remote-debugging-port` (loopback), `--remote-allow-origins=*`, and disable PNA/LNA blocking in `cef_runtime` so the public DevTools frontend can reach the loopback CDP endpoint.
- [x] 1.3 Add `src/devtools.rs`: fetch `/json/list` off the main thread, split objects by brace depth, pick the WhatsApp page target (skip about:blank/workers/frontend pages), return its absolute frontend URL.
- [x] 1.4 Add a DevTools view mode to `KarereWebView` (`new_devtools`): a normal OSR browser using a permissive client (`build_devtools_for`) that keeps navigations in-view; `open_devtools` loads the resolved frontend URL into it.
- [x] 1.5 Dock the DevTools view in the bottom child of a vertical `gtk::Paned` with a header (title + close button); on close remove the view, hide the pane, and force the main view to repaint.

## 2. Find handler

- [x] 2.1 Create `src/handlers/find.rs` with `wrap_find_handler!` building `ShellFindHandlerBuilder`.
- [x] 2.2 Implement `on_find_result(&self, _browser, identifier, count, selection_rect, active_match_ordinal, final_update)` writing `FindResult { count, active }` into `SharedState`.
- [x] 2.3 Add `find_handler` field to `KarereClient` in `src/handlers/client.rs` and override `Client::get_find_handler` to return it.
- [x] 2.4 Export the new module from `src/handlers/mod.rs`.

## 3. Search bar UI

- [x] 3.1 Update `data/ui/window.blp` to add a `gtk::SearchBar` directly under the headerbar and above the WebView container.
- [x] 3.2 Inside the SearchBar, add a `gtk::SearchEntry`, Previous and Next `gtk::Button`s, and a `gtk::Label` for the "n of m" counter.
- [x] 3.3 Wire `gtk::SearchBar::connect-entry` to the SearchEntry so the standard reveal/hide behavior works.

## 4. Find action and search bar wiring

- [x] 4.1 Add `win.find-in-page` GAction with accel `<Primary>f` that sets the SearchBar to revealed and grabs focus into the SearchEntry.
- [x] 4.2 Connect `SearchEntry::search-changed` to call `host.find(text, true, false, false)` on each change.
- [x] 4.3 Connect Next button to call `host.find(last_text, true, false, true)`; connect Previous to call `host.find(last_text, false, false, true)`; cache `last_text` between calls.
- [x] 4.4 Connect `Escape` key on the SearchEntry to hide the SearchBar and call `host.stop_finding(true)`.
- [x] 4.5 In the GTK polling tick, read `FindResult` from `SharedState` and update the counter label as "{active} of {count}" (or hide it when `count == 0`).

## 5. Manual verification

- [x] 5.1 Build and launch; press `F12` and confirm DevTools opens docked in the bottom pane (not a separate window) and renders the active page's DOM.
- [x] 5.2 Press `Ctrl+Shift+I` (and `F12` again) and confirm the pane toggles open/closed via both accels; drag the divider to resize.
- [x] 5.3 Interact with DevTools (click tabs, type in console) and confirm mouse + keyboard input reach the embedded inspector.
- [x] 5.4 Press `Ctrl+F`, type "chat" on WhatsApp Web, confirm matches highlight and the counter shows "n of m"; Next/Prev cycle both ways; `Escape` hides the bar and clears highlights.
- [x] 5.5 Confirm scrolling the page with the mouse wheel now actually scrolls the content under the cursor.
- [x] 5.6 Re-confirm main-view OSR rendering, input forwarding, clean shutdown, and that closing DevTools does not blank the main view.

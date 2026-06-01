## 1. RequestHandler skeleton

- [x] 1.1 Create `src/handlers/request.rs` and declare `ShellRequestHandlerBuilder` via `wrap_request_handler!`
- [x] 1.2 Add `request_handler: Arc<ShellRequestHandler>` field to `ShellClient` in `src/handlers/client.rs` and override `request_handler(&self)`
- [x] 1.3 Register the module in `src/handlers/mod.rs` and re-export the builder

## 2. External-link routing

- [x] 2.1 Implement `on_before_browse` to read `request.url()` and parse it once into scheme + host
- [x] 2.2 Allow `whatsapp.com`, `whatsapp.net`, `web.whatsapp.com`, and schemes `data:`, `blob:`, `about:`, `file:`, `chrome-error:` by returning `0`
- [x] 2.3 For everything else, call `gio::AppInfo::launch_default_for_uri(url, None::<&gio::AppLaunchContext>)` and return `1`
- [x] 2.4 Log `log::warn!` when `launch_default_for_uri` returns `Err` (codebase uses `log`, not `tracing`)

## 3. Renderer crash recovery

- [x] 3.1 Extend `SharedState` (in `src/handlers/mod.rs`) with `crash_toast: Option<String>`, `crash_history: Vec<Instant>`, and `pending_reload: Option<glib::SourceId>`
- [x] 3.2 Implement `on_render_process_terminated`: push timestamp, set `crash_toast = Some("Web view crashed — reconnecting…")`
- [x] 3.3 When fewer than 5 crashes in the last 60 s, `glib::timeout_add_local_once(Duration::from_millis(1500), || browser.reload())`
- [x] 3.4 When 5+ crashes in 60 s, set a `crash_dialog_request: Option<CrashDialog>` flag on `SharedState` instead of scheduling a reload
- [x] 3.5 New 100 ms poll loop in `src/window.rs` (`init_web_view`) drains `crash_toast` into the toast overlay and `crash_dialog_request` into an `AdwAlertDialog` with "Open logs" / "Cancel" actions

## 4. ContextMenuHandler

- [x] 4.1 Create `src/handlers/context_menu.rs` with `ShellContextMenuHandlerBuilder` via `wrap_context_menu_handler!`
- [x] 4.2 Add `context_menu_handler` field to `ShellClient` and override `context_menu_handler(&self)`
- [x] 4.3 Implement `on_before_context_menu` walking `model.count()` and `model.command_id_at(idx)` (cef-rs 148 method name)
- [x] 4.4 Remove the open-link entries in reverse index order. NOTE: cef-rs 148 has no `MENU_ID_OPEN_LINK_*` constants (`cef_menu_id_t` stops at `MENU_ID_VIEW_SOURCE`); pinned the Chromium IDC ids 50100/50101/50102 by value — re-verify on CEF upgrade
- [x] 4.5 Detect and remove separators that wrap the removed entries (leading/trailing/doubled)

## 5. LoadHandler retry/backoff

- [x] 5.1 Add `on_load_error(&self, _browser, _frame, error_code, _error_text, failed_url)` to `src/handlers/load.rs`
- [x] 5.2 Early-return when `error_code == Errorcode::ABORTED`
- [x] 5.3 Increment `SharedState.load_error_count`; backoff `min(60_000, 500 << retry)` with 0-based retry index → 500/1000/2000 ms (matches spec scenario)
- [x] 5.4 Schedule reload via `glib::timeout_add_local_once`, store the returned `SourceId` on `SharedState`
- [x] 5.5 In `on_load_end`, reset `load_error_count = 0` and cancel `pending_reload`

## 6. Verification

- [x] 6.1 Manual: external link → default browser opens, shell stays on WhatsApp. Fix: popups go through `LifeSpanHandler::on_before_popup` (+ `on_open_urlfrom_tab`), not `on_before_browse`; both now route via shared `route_target`
- [x] 6.2 Manual: right-click a link → no "Open in New Window/Tab/Incognito" entries (confirms pinned IDC ids 50100-50102 correct for this CEF build)
- [x] 6.3 Manual: kill renderer → toast + reload; 5 kills in 60 s → "Web view keeps crashing." dialog. NOTE: zygote-forked renderers show `--type=zygote`; set `KARERE_NO_ZYGOTE=1` to make them killable as `--type=renderer`
- [x] 6.4 Manual: drop network during initial load → `on_load_error` retries (500/1000/2000 ms), opaque "No connection" overlay covers the view, page lands on reconnect. Fix: ignore the `chrome-error://` error-page `on_load_end` so the retry/overlay survive
- [x] 6.5 `cargo clippy --workspace --all-targets -- -D warnings` clean and `cargo test --workspace` green (4/4); fixed pre-existing clippy debt in web_view.rs/cef_runtime.rs/actions.rs/application.rs

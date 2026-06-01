## 1. Cargo & module scaffolding

- [ ] 1.1 Add `serde = { version = "*", features = ["derive"] }`, `serde_json`, `uuid` (with `v4`), and `base64` to `Cargo.toml`.
- [ ] 1.2 Remove `cairo-rs`, `pango`, `pangocairo` from `Cargo.toml` (v3 custom-avatar renderer is gone).
- [ ] 1.3 Create `src/accounts.rs` and add `pub mod accounts;` to `src/lib.rs` (or `main.rs`).
- [ ] 1.4 Confirm `cargo build` succeeds with the new deps and no leftover references to dropped crates.

## 2. Account record and persistence

- [ ] 2.1 Define `AccountPermissions` (re-use M11 shape if available; otherwise stub struct with `Serialize`/`Deserialize`).
- [ ] 2.2 Define `Account` with all required fields (`id`, `wid`, `pushname`, `user_label`, `avatar_png`, `avatar_url`, `created_at`, `last_used_at`, `is_active`, `has_session`, `has_unread`, `zoom_level`, `permissions`); derive `Serialize`, `Deserialize`, `Clone`, `Debug`.
- [ ] 2.3 Compute the accounts root path: `$XDG_DATA_HOME/karere/accounts/` (fallback to `~/.local/share/karere/accounts/`).
- [ ] 2.4 Implement `AccountManager::load()` — returns empty list when `accounts.json` is missing; returns `Err` when present but malformed.
- [ ] 2.5 Implement `AccountManager::save()` — writes `accounts.json.tmp` then `fs::rename` to `accounts.json`.
- [ ] 2.6 Implement `add() -> Account` (UUID, set timestamps, append, persist), `remove(id)` (drop, persist, emit `accounts-changed`).
- [ ] 2.7 Implement `activate(id)` — set `last_used_at = now`, persist, emit `accounts-changed`.
- [ ] 2.8 Implement `get_accounts_sorted()` — clone list and `sort_by(|a, b| b.last_used_at.cmp(&a.last_used_at))`.
- [ ] 2.9 Implement `update_identity(id, wid, pushname)` and `update_avatar(id, png)` (persist on both).
- [ ] 2.10 Unit tests: atomic save survives mid-write simulation (write then crash before rename leaves prior file intact); MRU sort is correct for three accounts with shuffled `last_used_at`.

## 3. KarereWebView restructure

- [ ] 3.1 Rename `CefGtkArea` to `KarereWebView` (or wrap; whichever is less churn) and add `Mutex<HashMap<AccountId, Browser>>` + `Mutex<Option<AccountId>>` fields.
- [ ] 3.2 Implement `spawn_browser(account_id)`: build `RequestContextSettings { cache_path: sessions/<id>/data, ..Default::default() }`, create `RequestContext`, create browser pointed at `https://web.whatsapp.com`, insert into map, start in `was_hidden(true)` unless first account.
- [ ] 3.3 Implement `switch_to(new_id)`: `host(prev).was_hidden(true)`, set foreground = new_id, `host(new).was_hidden(false)`, `host(new).was_resized()`, `widget.queue_render()`.
- [ ] 3.4 Update `RenderHandler::view_rect` to early-return when `browser_id != foreground` (return cached foreground rect or zero rect when no foreground).
- [ ] 3.5 Update `RenderHandler::on_paint` to early-return when `browser_id != foreground` (no GL upload).
- [ ] 3.6 Wire input forwarding (M3) to dispatch only to the foreground browser.
- [ ] 3.7 Implement `close_account_browser(account_id)` for the `remove(id)` path.

## 4. IPC payloads (depend on M13 envelope)

- [ ] 4.1 Confirm `RendererMessage` includes `ProfileIdentity { wid, pushname }`, `ProfileAvatar { base64_png }`, `AwaitingPairing`, `StoreUnavailable { reason }` (M13 already declared these — add now if missing).
- [ ] 4.2 Add an optional `source: Option<String>` (or `source: Source` enum with `Store` / `DomFallback`) to the identity / avatar variants so the DOM fallback is distinguishable.
- [ ] 4.3 Browser-process dispatcher: route `ProfileIdentity` → `AccountManager::update_identity`; `ProfileAvatar` → base64-decode → `AccountManager::update_avatar`; `AwaitingPairing` → set row state; `StoreUnavailable` → set degraded flag + inject `data/js/profile_dom_fallback.js`.

## 5. Store hook (data/js/store_hook.js)

- [ ] 5.1 Implement `__webpack_require__` override that captures every module (wa-js technique). Wrap in `try/catch`; on catch emit `StoreUnavailable { reason: "<error msg>" }`.
- [ ] 5.2 Locate `Store` namespace; verify presence of `Store.Conn` and `Store.ProfilePicThumb`; emit `StoreUnavailable` if either missing.
- [ ] 5.3 Subscribe to `Store.Conn`; on first `wid` + `pushname` availability, emit `RendererMessage::ProfileIdentity`.
- [ ] 5.4 Avatar fetch: `Store.ProfilePicThumb.find(Store.Conn.wid)` → `fetch(desc.eurl)` → `.blob()` → `FileReader.readAsDataURL` → strip `data:image/png;base64,` prefix → emit `RendererMessage::ProfileAvatar { base64_png }`.
- [ ] 5.5 Subscribe to `Store.Contact` self-wid for pic-changed events; re-run step 5.4.
- [ ] 5.6 Subscribe to `Store.AppState` state changes; emit `AwaitingPairing` while `state !== 'CONNECTED'` (debounce 500 ms); stop emitting once `CONNECTED`.

## 6. DOM fallback (data/js/profile_dom_fallback.js)

- [ ] 6.1 File is NOT injected by the M13 default bundle; instead, the browser process calls `frame.execute_java_script` on receipt of `StoreUnavailable`.
- [ ] 6.2 Poll `#side header img` for `blob:` `src` (max 1 Hz); when found, convert to base64 PNG and emit `ProfileAvatar { base64_png, source: "dom-fallback" }`.
- [ ] 6.3 Read `#side header span[dir="auto"][title]` for pushname; emit `ProfileIdentity { wid: null, pushname, source: "dom-fallback" }`.
- [ ] 6.4 Keep degraded flag persistent — do NOT emit a "store-restored" signal from this script.

## 7. Switcher UI

- [ ] 7.1 Create `data/ui/account_switcher.blp` with `Adw.BottomSheet` + `Gtk.Popover` variants (mirror karere `window.blp:85-122`).
- [ ] 7.2 Define a row template using `Adw.Avatar` (custom-image OR initials based on `avatar_png`); subtitle reads pushname or "Waiting for QR scan…" while `AwaitingPairing`.
- [ ] 7.3 Add a yellow badge widget controlled by an `is_degraded: bool` per-row property.
- [ ] 7.4 Bind the row list to `AccountManager::get_accounts_sorted()`; re-bind on every `accounts-changed` signal.
- [ ] 7.5 On row activation, call `AccountManager::activate(id)` and `KarereWebView::switch_to(id)`.
- [ ] 7.6 Verify no reorder UI is present (no drag handle, no up/down arrow, no context-menu reorder action).

## 8. Add/edit dialog

- [ ] 8.1 Port the dialog shape from `karere/src/window.rs:2597-2829`; remove emoji picker and color picker rows.
- [ ] 8.2 Editable widgets: `user_label` entry only. Display-only (greyed): `wid`, `pushname`, `avatar_url` preview.
- [ ] 8.3 "Add account" action: create `Account` via `AccountManager::add`, spawn its hidden browser, open dialog; dialog closes when `ProfileIdentity` arrives or user cancels.
- [ ] 8.4 "Edit account" action: open dialog populated from existing account; Save persists `user_label` only.
- [ ] 8.5 "Remove account" action: confirm via `Adw.MessageDialog`, then call `AccountManager::remove(id)` + `KarereWebView::close_account_browser(id)`.

## 9. Tray integration (M15 dependency)

- [ ] 9.1 Re-render tray menu on every `accounts-changed` signal.
- [ ] 9.2 Each tray account entry uses the same avatar bytes as the switcher (right-click tray shows photos).

## 10. Documentation

- [ ] 10.1 Update CHANGELOG / release notes: "v4 hard-fork; existing accounts must be re-linked."
- [ ] 10.2 Module-level rustdoc on `src/accounts.rs` describing JSON layout, atomic-write guarantee, MRU contract.
- [ ] 10.3 Short README block on the Store hook's degraded-mode contract (badge persists until next Store success).

## 11. Verification

- [ ] 11.1 Link 2 accounts; switcher shows correct names + photos within 30 s of pairing.
- [ ] 11.2 Switching swaps cookies — logout in one ≠ logout in other (verified by DevTools `document.cookie` after switch).
- [ ] 11.3 MRU ordering: activate B, then A, then C → switcher order = C, A, B.
- [ ] 11.4 Right-click tray (M15) lists both accounts with their photos.
- [ ] 11.5 Force Webpack hook failure (in DevTools rename `window.Store` to undefined and reload) → degraded-mode yellow badge appears; DOM fallback eventually populates name + avatar; badge stays yellow.
- [ ] 11.6 Background-browser pause: open two accounts, observe CPU drop on the non-foreground browser via `top`/`htop` after switch.
- [ ] 11.7 Atomic save: kill the process between the `.tmp` write and the rename (use a debug breakpoint); confirm prior `accounts.json` is intact on restart.

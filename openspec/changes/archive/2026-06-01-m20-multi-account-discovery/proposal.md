## Why

Karere v3 supports multi-account via WebKit's per-`WebView` `WebsiteDataManager`, but identity (name + avatar) was scraped from the DOM after sign-in and accounts were ordered by a user-editable `order` field. v4 (CEF, post-M7 hard-fork) needs the equivalent isolation primitive (`RequestContext` per account, distinct `cache_path`) plus a first-class identity source: WhatsApp Web's `Store` (via the same `__webpack_require__` hook `@wppconnect/wa-js` uses). DOM scraping is retained only as a degraded fallback for upstream Webpack churn. Order is locked to MRU (`last_used_at` desc) to remove a footgun feature.

## What Changes

- New `src/accounts.rs` defining `Account` struct (id, optional `wid`/`pushname`, user-editable `user_label`, decoded `avatar_png`, `avatar_url`, timestamps, `is_active`, `has_session`, `has_unread`, `zoom_level`, `permissions`) and `AccountManager` with `load` / `save` / `add` / `remove` / `activate` / `get_accounts_sorted` / `update_identity` / `update_avatar`. JSON store at `$XDG_DATA_HOME/karere/accounts/accounts.json` with atomic temp-then-rename writes.
- Restructure `cef_gtk_area.rs` (renamed `KarereWebView`) to hold `Mutex<HashMap<AccountId, Browser>>` + `Mutex<Option<AccountId>>` foreground pointer; per-account `RequestContext` from `RequestContextSettings { cache_path: $XDG_DATA_HOME/karere/accounts/sessions/<id>/data, ..Default::default() }`; background browsers paused with `host.was_hidden(true)`; `view_rect`/`on_paint` early-return when `browser_id != foreground`.
- `data/js/store_hook.js` injected at `on_context_created` (main world) hooks `__webpack_require__` to obtain `window.Store`, subscribes to `Store.Conn` for `wid`/`pushname`, resolves `Store.ProfilePicThumb.find(...).eurl` to a base64 PNG via `fetch().blob() → FileReader`, and subscribes to `Store.Contact(self-wid)` for pic-changed re-fetch. On hook failure posts `RendererMessage::StoreUnavailable { reason }`.
- `data/js/profile_dom_fallback.js` loaded only after `StoreUnavailable`, polling `#side header img` / `span[dir="auto"][title]`. IPC tagged `source: "dom-fallback"`. Switcher row keeps yellow "degraded mode" badge even when fallback succeeds.
- Pairing state derived from `Store.AppState.state === 'CONNECTED'`; pre-connected → `RendererMessage::AwaitingPairing` → switcher row "Waiting for QR scan…".
- `data/ui/account_switcher.blp` ported from karere `window.blp:85-122` bottom-sheet + popover; uses `Adw.Avatar` with `custom-image = gdk::Texture::from_bytes(avatar_png)` when present, else `show-initials=true` driven by `user_label.or(pushname).unwrap_or("?")` and Adw.Avatar built-in color hashing.
- Add/edit dialog ported from `karere/src/window.rs:2597-2829` shape, dropping emoji + color fields; only `user_label` is editable, auto-discovered fields displayed greyed-out.
- `Cargo.toml`: add `serde` (with `derive`), `serde_json`, `uuid`, `base64`; drop `cairo-rs`, `pango`, `pangocairo` (v3 custom-avatar renderer gone).
- v3 → v4 migration: documented as "re-link required"; no migration code.

## Capabilities

### New Capabilities
- `account-manager`: Persistent `Account` records + `AccountManager` CRUD with MRU ordering and atomic JSON persistence.
- `cef-browser-pool`: Per-account `RequestContext`-isolated CEF browsers with foreground/background switching and OSR paint gating.
- `account-auto-discovery`: First-class `Store`/Webpack identity hook plus degraded DOM fallback for name + avatar.
- `account-switcher-ui`: MRU-ordered switcher (bottom-sheet on mobile, popover on desktop) with Adw.Avatar rendering and add/edit dialog.

### Modified Capabilities
<!-- none -->

## Impact

- `src/accounts.rs` (new): `Account`, `AccountPermissions`, `AccountManager`, atomic JSON I/O.
- `src/cef_gtk_area.rs` → `KarereWebView`: browser map, foreground pointer, per-account `RequestContext`, `was_hidden` pause, OSR gating.
- `src/window.rs`: switcher binding, add/edit dialog wiring, MRU `activate()` calls on user selection.
- `data/js/store_hook.js` (new), `data/js/profile_dom_fallback.js` (new), loaded via the M13 bundle pipeline.
- `data/ui/account_switcher.blp` (new); reuses Adw.Avatar instead of v3 custom Cairo/Pango renderer.
- `Cargo.toml`: add `serde`, `serde_json`, `uuid`, `base64`; drop `cairo-rs`, `pango`, `pangocairo`.
- Depends on M13 (IPC envelope ships `ProfileIdentity` / `ProfileAvatar` / `AwaitingPairing` / `StoreUnavailable`), M15 (tray entries listed per account), M18 (`Account::zoom_level` consumer).
- Non-goals: cross-account chat aggregation, account-specific user-agent, migration code from v3 accounts.json.

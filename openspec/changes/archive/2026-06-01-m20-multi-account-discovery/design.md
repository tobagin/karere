## Context

Karere v3 supported multiple WhatsApp Web accounts by allocating one `WebKitWebView` per account, each bound to a distinct `WebsiteDataManager` rooted at `$XDG_DATA_HOME/karere/accounts/<id>/`. Account identity (display name, avatar) was harvested from the DOM (`#side header img`, `header span[title]`) after sign-in completed, and accounts were rendered in a user-editable order field (`order: i32`) — an explicit per-row drag handle in the v3 switcher.

CEF's isolation primitive is `RequestContext`, not `WebView`, and CEF's `Store`-equivalent on WhatsApp Web is reachable through Webpack internals via the same `__webpack_require__` hook `@wppconnect/wa-js` uses. The v4 hard-fork (M7) opens the door to (a) replacing DOM scraping with the Store hook, (b) replacing user-editable ordering with MRU (`last_used_at` desc), and (c) reusing Adw.Avatar instead of the v3 custom Cairo/Pango renderer. M13 already provides the IPC envelope; M18 already references `Account::zoom_level`.

## Goals / Non-Goals

**Goals:**
- One `RequestContext` per account; per-account `cache_path` under `$XDG_DATA_HOME/karere/accounts/sessions/<id>/data`; cookies and storage strictly isolated.
- Single OSR surface (one `GtkGLArea`-equivalent); switching accounts is a foreground-pointer swap, not a widget swap.
- Background browsers paused via `BrowserHost::was_hidden(true)`; `on_paint` callbacks for background browsers discarded via early-return on `browser_id != foreground`.
- `Store` (Webpack) hook is the first-class identity source. DOM fallback only on `StoreUnavailable`.
- MRU ordering only; no `order` field, no drag handles.
- Avatars auto-discovered and re-fetched on `Store.Contact` pic-changed events; user can override the label via `user_label` but not the auto-fields.

**Non-Goals:**
- Cross-account chat aggregation (separate browsers, separate Store instances; no shared inbox).
- Account-specific user-agent strings.
- Code-path migration of v3 `accounts.json` records. Re-link required; documented in release notes.
- Per-account theming or custom colors (v3 had emoji + color fields; both dropped).

## Decisions

### Decision: `RequestContext` per account, single OSR surface
- **Choice**: `KarereWebView` owns `Mutex<HashMap<AccountId, Browser>>` and `Mutex<Option<AccountId>>` (the foreground id). Each browser is created via `RequestContext::new(&RequestContextSettings { cache_path: $XDG_DATA_HOME/karere/accounts/sessions/<id>/data, ..Default::default() })`. Only the foreground browser's paint output is uploaded to the GL texture; `RenderHandler::view_rect` and `on_paint` early-return when `browser_id != foreground`.
- **Why**: `RequestContext` is CEF's only true isolation boundary for cookies/storage. A single OSR surface keeps GTK widget tree small and avoids re-layout on switch; the early-return gate prevents wasted GPU uploads from hidden browsers that still tick timers.
- **Alternatives**: (a) One `GtkGLArea` per account, swap widget on activate — heavier, breaks input-forwarding map (M3), and re-allocates the GL context. (b) Suspend background browsers via `close_browser` — would log them out / require re-pair on switch.

### Decision: Pause background browsers via `was_hidden(true)`
- **Choice**: On switch: `host(prev).was_hidden(true)`; foreground pointer = new id; `host(new).was_hidden(false)`; `host(new).was_resized()`; `widget.queue_render()`.
- **Why**: CEF's documented background-throttle hook. Avoids closing browsers (which would log out) while still letting Chromium tick down timers and lower priority.
- **Risk**: Notifications still must fire from background browsers (M14 dependency). M14's `RendererMessage::NotificationSeen` is emitted regardless of `was_hidden`, so this is safe.

### Decision: MRU order, no user reorder
- **Choice**: `Account::last_used_at: i64`. `AccountManager::get_accounts_sorted()` returns `sort_by(|a, b| b.last_used_at.cmp(&a.last_used_at))`. `activate(id)` sets `last_used_at = now`, persists, emits `accounts-changed`.
- **Why**: Removes a user footgun (drag-reorder in a list whose entries are rarely touched leads to "lost" accounts) and the storage migration risk of stable ordering across versions. MRU surfaces the account the user just used at the top, which matches every other modern account-switcher (browser profile menu, OS user switcher).
- **Locked**: This was an explicit user decision; do not add an `order` field "for later".

### Decision: Store (Webpack) hook is first-class; DOM fallback is degraded
- **Choice**: `data/js/store_hook.js` (injected by M13 bundle, main world, at `on_context_created`) overrides `__webpack_require__` to capture every module, finds the `Store` namespace (`wa-js` technique), subscribes to `Store.Conn` for `wid`/`pushname` and to `Store.Contact(self-wid)` for pic-changed events. Avatar fetched via `Store.ProfilePicThumb.find(wid).eurl → fetch → blob → FileReader.readAsDataURL → base64 PNG`. On any hook failure (Webpack restructure, throw inside the hook, missing namespace), the script posts `RendererMessage::StoreUnavailable { reason }`. Only then does the browser process inject `data/js/profile_dom_fallback.js`, which polls `#side header img` and `header span[dir="auto"][title]`. The fallback's IPC messages carry `source: "dom-fallback"` and the switcher row shows a yellow "degraded mode" badge that does NOT clear when the fallback succeeds — only restoring the Store hook (typically on next WhatsApp release) clears it.
- **Why**: DOM scraping was the v3 source of breakage; surface that risk visibly to nudge upstream-hook maintenance.
- **Alternatives**: (a) Always DOM-scrape — gives up the determinism and the connected-state signal. (b) Drop DOM fallback entirely — leaves users with no name/avatar across a Webpack churn window. The badge is the compromise.

### Decision: Pairing state derived from Store, not URL
- **Choice**: While `Store.AppState.state !== 'CONNECTED'`, post `RendererMessage::AwaitingPairing` (debounced). Switcher row reads "Waiting for QR scan…" with the spinner.
- **Why**: URL-based detection (`web.whatsapp.com/qr`) is fragile and was the v3 source of "stuck on QR" UI bugs. Store's `AppState` is the authoritative signal.

### Decision: JSON store with atomic temp-then-rename
- **Choice**: Persist `Vec<Account>` to `$XDG_DATA_HOME/karere/accounts/accounts.json` by writing to `accounts.json.tmp` then `fs::rename`. `serde_json` + `serde` derive.
- **Why**: Matches v3 storage layout (zero learning curve), survives mid-write crashes (rename is atomic on the same filesystem), and avoids pulling sqlite or a TOML-vs-JSON debate. Avatars stored as base64 in the same JSON (small — ~96 px PNG, typically <16 KB).
- **Alternatives**: (a) Sqlite — overkill for ≤10 accounts. (b) One file per account — increases fsync count without benefit.

### Decision: Adw.Avatar replaces v3 custom renderer
- **Choice**: Switcher and dialog rows use `Adw.Avatar`. When `avatar_png` present: `custom-image = gdk::Texture::from_bytes(&Bytes::from(&avatar_png))`. Otherwise `show-initials = true`, `text = user_label.or(pushname).unwrap_or("?")`, letting Adwaita's color hash pick a tint.
- **Why**: Drops the v3 Cairo/Pango dependency chain (`cairo-rs`, `pango`, `pangocairo`) and inherits libadwaita's accessibility + RTL handling for free.

## Risks / Trade-offs

- **Risk**: Webpack restructure in a future WhatsApp release breaks the Store hook. → Mitigation: degraded-mode badge + DOM fallback keep the UI usable while a hook patch lands.
- **Risk**: Base64-in-JSON avatar storage bloats `accounts.json` if WhatsApp moves to larger thumbnails. → Acceptable through 256 px (still <64 KB/account). If thumbnails grow, switch to one PNG file per account under `sessions/<id>/avatar.png`; record-shape stays the same (`avatar_png: Option<Vec<u8>>` becomes a lazy load).
- **Risk**: `was_hidden(true)` on background browsers degrades notification latency. → Empirically Chromium still ticks background tasks; M14 notifications fire. Verify in M14 acceptance.
- **Risk**: Atomic temp-then-rename is not atomic across filesystems. → Mitigation: temp file lives in the same directory as the target; `$XDG_DATA_HOME` is one filesystem in practice.
- **Trade-off**: No migration code from v3 means existing users must re-link every account. → Locked decision; release notes call it out.
- **Trade-off**: MRU means a rarely-used account can sink to the bottom and feel "hidden". → Acceptable; switcher lists ≤10 entries, all visible at once.

## Migration Plan

v3 → v4 is a hard-fork; no migration code. Existing `$XDG_DATA_HOME/karere/accounts/accounts.json` is ignored. Re-link required. Release notes must call this out explicitly.

## Open Questions

- None blocking. Future work (out of scope here): per-account user-agent override for diagnosing WhatsApp A/B rollouts; cross-account unread badge aggregation.

## Why

M5 wired a permission prompt for CEF media-access requests, but the dialog fires on every call: a Karere-style WhatsApp session re-asks for microphone, camera, notifications, and geolocation after each reload. Karere v3 had per-account remembered decisions (`accounts.rs:255-294`, `window.rs:1300-1418`) which the shell currently lacks. Additionally, M5 only covers `on_request_media_access_permission`; CEF's separate `on_show_permission_prompt` path (notifications, geolocation, midi-sysex, clipboard-read) still falls through to default deny. Until M20 introduces multi-account support, decisions can live in a single global GSettings dict keyed by origin and CEF permission-mask bit; M20 will migrate this into per-account JSON.

## What Changes

- Extend `src/handlers/permission.rs` with `on_show_permission_prompt(&self, _browser, prompt_id: u64, requesting_origin, requested_permissions: u32, callback)` returning `1` (async), and route through the same persistence + dialog flow as the media path.
- Consult `permissions_store::get(origin, mask)` at the top of `on_request_media_access_permission` and `on_show_permission_prompt` — `Allow` short-circuits to `callback.cont(CEF_PERMISSION_RESULT_ACCEPT)`, `Deny` to `cont(CEF_PERMISSION_RESULT_DENY)`, `AskMixed` falls through to a forced prompt (no stored bits are reused).
- Add `src/permissions_store.rs` exposing `Decision { Allow, Deny, AskAll, AskMixed }`, `get(origin, mask) -> Decision` and `set(origin, mask, decision)` backed by a GSettings dict `permission-decisions` of type `a{s a{ui}}` (origin → bit → state where `0=Ask`, `1=Allow`, `2=Deny`; inner value type `i`/i32). Default state is `Ask`; every concrete Allow/Deny is persisted automatically (browser-style).
- Extend the AdwAlertDialog body composed from `describe_permissions(mask)` (notifications / location / midi / clipboard included). No "remember" checkbox — the decision is always remembered.
- Add `permission-decisions` to the GSchema with default `{}`.

## Capabilities

### New Capabilities
- `permission-store`: durable per-origin permission decisions backed by GSettings, queried before any prompt and written when the user opts to remember.

### Modified Capabilities
- `permission-prompt-extended`: extends the M5 `permission-prompt` capability to cover `on_show_permission_prompt` (notifications / geolocation / midi-sysex / clipboard-read), consults the store before showing the dialog, and adds the "Remember this choice" checkbox.

## Impact

- New file: `src/permissions_store.rs`.
- Modified: `src/handlers/permission.rs` (new method, store lookups, checkbox in dialog body, broader `describe_permissions`), `src/handlers/mod.rs` (module wiring for the store), `data/<app-id>.gschema.xml` (new key), `src/main.rs` or wherever GSettings is initialised (none expected — `gio::Settings::new` is created on demand).
- No new crate dependencies.
- Non-goals: per-account scoping (lands in M20 when `accounts.rs::AccountPermissions` exists), settings UI to clear stored decisions, localisation of dialog strings.

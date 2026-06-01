## Context

M5 shipped `ShellPermissionHandler` with a stateless dialog for `on_request_media_access_permission`. The handler returns `1` and resolves the callback from a `spawn_local` block after the user picks Allow/Deny. CEF additionally invokes `on_show_permission_prompt` for notifications, geolocation, midi-sysex and clipboard-read; the M5 handler does not override it, so those requests fall through to CEF's default deny. Karere v3 persisted decisions per-account in a JSON blob loaded at startup (`accounts.rs:255-294`, `window.rs:1300-1418`); pre-M20 the shell has no account abstraction, so a single global GSettings dict is the most practical interim store.

`adw::AlertDialog` supports an "extra child" via `set_extra_child`, which we use to host a `gtk::CheckButton` for "Remember this choice".

## Goals / Non-Goals

**Goals:**
- Cover both `on_request_media_access_permission` and `on_show_permission_prompt` with the same prompt flow.
- Persist Allow/Deny per `(origin, permission-bit)` and short-circuit future requests for fully-stored masks.
- Never auto-allow: a fresh request always defaults to Ask, regardless of what other origins answered.
- Keep the store decoupled from `ShellPermissionHandler` (separate module) so M20 can swap the backend to per-account JSON without touching the handler.

**Non-Goals:**
- Per-account scoping. M20 owns that migration once `accounts.rs::AccountPermissions` lands.
- A settings UI to inspect/clear stored decisions; users can still `gsettings reset <app-id> permission-decisions`.
- Localisation of dialog strings or permission labels.
- Granular partial-grant from a mixed dialog (we re-prompt for the full mask when even one bit is unstored).

## Decisions

- **Two-level GSettings dict `a{s a{ui}}`.** Outer key is the origin string (`https://web.whatsapp.com`), inner key is the raw CEF permission-mask bit (so one bit per entry). Values are `0=Ask`, `1=Allow`, `2=Deny`. Storing per-bit (rather than per-mask) lets partial overlaps work — if WhatsApp previously stored microphone=Allow, a later combined mic+camera request can recognise mic as Allow and still prompt for camera. The decision resolution logic in `get` walks every bit in the requested mask and returns:
  - `Allow` if every requested bit is stored as Allow,
  - `Deny` if every requested bit is stored as Deny,
  - `AskAll` if every requested bit is Ask (no stored entries),
  - `AskMixed` otherwise.
  Both `AskAll` and `AskMixed` force a prompt; the distinction exists so callers can log/telemeter the "mixed" case without changing UX.

- **AskMixed re-prompts for the full mask.** When some bits are stored and others are not, we discard the stored bits and ask the user about everything in the request. Rationale: showing a partial dialog ("camera was previously allowed, please decide microphone") is confusing and the user may want to reconsider the older grant in light of the new combined request. This matches the verify scenario in the milestone.

- **Every Allow/Deny is remembered (browser-style).** There is no "Remember this choice" checkbox: like a normal browser, a granted/denied permission is persisted immediately so the prompt never re-fires for that origin+mask. We still do not write `Ask` rows (no distinction between "explicitly unset" and "never set", and it keeps the dict small).
- **Inner value type is `i` (i32).** The GSettings key is `a{sa{ui}}`, so the stored state is `i32` (`0`/`1`/`2`). The Rust store type must be `HashMap<String, HashMap<u32, i32>>` — using `u32` produces `a{sa{uu}}`, which silently fails the variant-type check on both read and write (nothing persists).

- **Store lives in a standalone module.** `src/permissions_store.rs` exposes `get`, `set`, and the `Decision` enum. It owns its own `gio::Settings` handle constructed lazily. This keeps `permission.rs` focused on dialog plumbing and lets M20 reimplement the store against `AccountPermissions` without churning the handler.

- **Reuse the M5 async pattern.** Both handler methods return `1` and trampoline through `glib::MainContext::default().spawn_local`. The store lookup happens before the trampoline so an `Allow`/`Deny` short-circuit resolves the callback synchronously (still returning `1` to satisfy CEF's async contract; we call `callback.cont` immediately and return).

- **`describe_permissions` grows.** It already covers camera / microphone / location / notifications / clipboard. We add midi-sysex (`CEF_PERMISSION_TYPE_MIDI_SYSEX`) and ensure clipboard-read is distinguishable from generic clipboard if CEF exposes the bits separately; otherwise the existing "clipboard" label is reused.

## Risks / Trade-offs

- **Chromium also persists permissions (two stores).** Chromium keeps its own per-origin content settings in the CEF profile/cache. Once a permission is granted/blocked there, Chromium stops invoking our handler, so our GSettings store is bypassed and `dconf reset permission-decisions` alone does NOT return the origin to Ask — the CEF cache's site setting must also be cleared. Our store therefore takes effect only when Chromium would itself prompt; its value is the consistent Karere dialog + a backend M20 can migrate. If full authority is needed later, force Chromium to always-ask via `RequestContext` preferences so our store becomes the source of truth.

- **Single-account assumption.** Storing decisions globally means switching to a second WhatsApp account (post-M20) would share permissions with the first until M20 migrates the data. Mitigation: M20's migration step reads `permission-decisions`, fans it out per account, and clears the global key.

- **GSettings dict ergonomics.** `a{s a{ui}}` is awkward to mutate atomically; concurrent writes from two windows could race. In practice the shell is single-process and the store is touched from the glib main thread only, so the race is theoretical.

- **AskMixed UX.** Re-prompting for a full mask when only one bit is new could annoy users who already allowed the other bit. Mitigation: the verify scenario explicitly tests this; revisit if telemetry (post-M20) shows it as a common path.

- **Schema migration.** Adding `permission-decisions` to the GSchema requires `glib-compile-schemas` to be rerun at install time; on Flatpak this happens automatically, on local dev users need to recompile their schema dir. Build scripts already handle this for other keys.

- **Unknown permission bits.** New bits added upstream are stored under their raw integer value; resolution still works but `describe_permissions` will show "device access" until extended. Same caveat as M5.

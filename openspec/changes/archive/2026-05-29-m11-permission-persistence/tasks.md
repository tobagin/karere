## 1. Permission store

- [x] 1.1 Add `permission-decisions` GSetting of type `a{sa{ui}}` (origin → mask-bit → state) in `data/io.github.tobagin.karere.gschema.xml.in`
- [x] 1.2 Create `src/permissions_store.rs` with `Decision { Allow, Deny, AskAll, AskMixed }` and `State` enum (`Ask=0`, `Allow=1`, `Deny=2`)
- [x] 1.3 Implement `get(origin: &str, mask: u32) -> Decision`: explode mask into single-bit lookups; reduce to Allow/Deny/AskMixed
- [x] 1.4 Implement `set(origin: &str, mask: u32, decision: Decision)`: persist per-bit `i32` state for concrete Allow/Deny (always remembered); never write `Ask`
- [x] 1.5 Unit-test the bit reduction logic (Allow ∩ Deny → AskMixed, partial overlap → AskMixed, full match → Allow/Deny)

## 2. Extend permission handler

- [x] 2.1 In `src/handlers/permission.rs`, add `on_show_permission_prompt(&self, _browser, prompt_id, requesting_origin, requested_permissions, callback)`
- [x] 2.2 Consult `permissions_store::get(origin, mask)`; Allow → `cb.cont(CEF_PERMISSION_RESULT_ACCEPT)`, Deny → `cb.cont(CEF_PERMISSION_RESULT_DENY)`, both return 1
- [x] 2.3 Ask/AskMixed → spawn_local AdwAlertDialog with body composed from `describe_permissions(mask)` (no remember checkbox — always remembered)
- [x] 2.4 Persist via `permissions_store::set(origin, mask, choice)` and `cb.cont(result)`
- [x] 2.5 Reuse same flow inside `on_request_media_access_permission` — consult store first, skip dialog when persisted

## 3. Wire dialog UX

- [x] 3.1 `describe_permissions(mask)` already exists in M5 — extend for non-media types (notifications, geolocation, midi-sysex, clipboard-read)
- [x] 3.2 AlertDialog labels: "Allow"/"Deny" only; default + close response = Deny
- [x] 3.3 Document that AskMixed forces full reconfirmation when an origin requests a wider mask than previously persisted

## 4. Verify

- [x] 4.1 WhatsApp mic request → Allow → restart → no prompt (always remembered)
- [x] 4.2 WhatsApp notifications request → Deny → restart → silently denied
- [x] 4.3 Mic+camera request after only mic stored → dialog re-prompts (AskMixed)
- [x] 4.4 Reset to Ask state requires clearing BOTH layers: `dconf reset /io/github/tobagin/karere/permission-decisions` (M11 store) AND Chromium's own per-origin content setting (clear the CEF cache/site data) — dconf alone is insufficient because Chromium persists granted permissions in its profile and stops invoking the handler.

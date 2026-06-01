## ADDED Requirements

### Requirement: Identity is read from WhatsApp's persisted storage
The system SHALL discover each account's own identity (wid, display name, avatar)
by reading the data WhatsApp Web already persists, injected via the M13 bundle
(`data/js/60-store-hook.js`) into every main frame at `on_context_created`. It
SHALL NOT depend on WhatsApp's Webpack/`Store` internals.

> Rationale: WhatsApp restores the native `Array.push` on its Webpack chunk
> array after load, so the `@wppconnect/wa-js` `__webpack_require__` capture is
> unreachable. The persisted storage below is stable and present once paired.

#### Scenario: WID from localStorage
- **WHEN** the account is paired
- **THEN** the script reads `localStorage['last-wid-md']` (e.g. `"353830357840:19@c.us"`) and derives the canonical wid `353830357840@c.us`

#### Scenario: Display name from IndexedDB
- **WHEN** the script resolves the account name
- **THEN** it reads the `model-storage` database `contact` object store and takes `name` (falling back to `pushname` / `notify` / `verifiedName`) from the row whose `id` matches the signed-in user
- **AND** posts `RendererMessage::ProfileIdentity { wid, pushname, source: "store" }`

#### Scenario: Avatar fetched in-page to base64
- **WHEN** the script resolves the avatar
- **THEN** it reads the cached URL `localStorage['WACachedProfilePicEURL']`, `fetch`es it (CORS-OK; `pps.whatsapp.net` serves `image/jpeg`), reads the blob via `FileReader.readAsDataURL`, strips the `data:…;base64,` prefix, and posts `RendererMessage::ProfileAvatar { base64_png, source: "store" }`

#### Scenario: Re-emit on change only
- **WHEN** the script polls on its interval
- **THEN** it re-emits identity/avatar only when the resolved value changed (de-duped), so a steady state produces no repeated IPC

### Requirement: Pairing state without StoreUnavailable
The system SHALL signal pairing state from the presence of the persisted wid and
SHALL NOT emit `StoreUnavailable` during normal operation.

#### Scenario: AwaitingPairing before login
- **WHEN** `localStorage['last-wid-md']` is absent (not yet paired)
- **THEN** the script posts `RendererMessage::AwaitingPairing` (debounced) and keeps polling
- **AND** it does NOT post `StoreUnavailable` (no degraded badge for an unpaired account)

#### Scenario: Identity clears awaiting + degraded
- **WHEN** a `ProfileIdentity { source: "store" }` arrives for an account
- **THEN** the account's awaiting-pairing flag is cleared
- **AND** any prior degraded flag is cleared (a store-sourced identity is a successful attachment)

### Requirement: Degraded DOM fallback path is retained
The system SHALL keep a degraded DOM-scrape fallback wired to `StoreUnavailable`
for resilience, even though the storage reader above does not emit it in normal
operation.

#### Scenario: Fallback injected on StoreUnavailable
- **WHEN** the browser process receives `RendererMessage::StoreUnavailable { reason }` for an account (first transition only)
- **THEN** it injects `data/js-deferred/profile_dom_fallback.js` into that account's main frame
- **AND** marks the account degraded (a persistent yellow badge on its switcher row)

#### Scenario: Degraded badge persists through fallback success
- **GIVEN** an account is degraded
- **WHEN** the DOM fallback reports identity/avatar with `source: "dom-fallback"`
- **THEN** the badge remains until a later `source: "store"` identity clears it

### Requirement: Idempotent runtime-state signalling
The system SHALL emit the `accounts-changed` signal for transient runtime flags
(awaiting-pairing, degraded) only on an actual state transition.

#### Scenario: No rebuild storm
- **WHEN** the renderer reports the same runtime state repeatedly (e.g. many `AwaitingPairing` in a second)
- **THEN** `accounts-changed` is emitted at most once per real change, so the switcher is not rebuilt continuously

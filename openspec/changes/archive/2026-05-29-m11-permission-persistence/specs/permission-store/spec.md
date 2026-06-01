## ADDED Requirements

### Requirement: Persistent per-origin permission decisions
The shell SHALL persist user permission decisions per `(origin, permission-mask-bit)` in a GSettings dict `permission-decisions` of type `a{s a{ui}}`, where the inner value is `0` (Ask), `1` (Allow), or `2` (Deny). The default state for any unstored bit MUST be Ask, and the shell MUST NOT auto-allow any request whose decision has not been explicitly recorded.

#### Scenario: Persisted Allow short-circuits the prompt
- **WHEN** a user previously chose Allow for microphone on `https://web.whatsapp.com`
- **AND** the page later requests microphone again (in any session)
- **THEN** `permissions_store::get("https://web.whatsapp.com", MIC_STREAM)` returns `Decision::Allow`
- **AND** the permission handler calls `callback.cont(CEF_PERMISSION_RESULT_ACCEPT)` without showing a dialog

#### Scenario: Persisted Deny short-circuits the prompt
- **WHEN** a user previously chose Deny for notifications on `https://web.whatsapp.com`
- **AND** the page later requests notifications again
- **THEN** `permissions_store::get("https://web.whatsapp.com", NOTIFICATIONS)` returns `Decision::Deny`
- **AND** the permission handler calls `callback.cont(CEF_PERMISSION_RESULT_DENY)` without showing a dialog

#### Scenario: Unstored bit returns Ask
- **WHEN** an origin has no row in `permission-decisions`, or a row exists but the requested bit is absent or set to `0`
- **THEN** `get` reports `Decision::AskAll` (when every requested bit is unset) so the handler shows the dialog

#### Scenario: Partial overlap returns AskMixed
- **WHEN** an origin has microphone stored as Allow but no entry for camera
- **AND** the page requests a combined `microphone | camera` mask
- **THEN** `get` returns `Decision::AskMixed`
- **AND** the handler shows the full prompt covering both bits, ignoring the stored microphone Allow

### Requirement: Decisions persist automatically
The shell SHALL persist every concrete Allow/Deny decision (browser-style), so a granted or denied permission is never re-asked for that origin. `Ask` states MUST NOT be written, keeping the dict free of empty rows. The inner GSettings value type is `i` (i32: `0`=Ask, `1`=Allow, `2`=Deny).

#### Scenario: Allow persists across restart
- **WHEN** the user picks Allow for microphone on `https://web.whatsapp.com`
- **THEN** `permissions_store::set(origin, MIC_STREAM, Decision::Allow)` writes `1` for that bit under the origin
- **AND** after the app restarts, the next microphone request from the same origin returns `Decision::Allow` and shows no dialog

#### Scenario: Allow writes every requested bit
- **WHEN** the user picks Allow for a `microphone | camera` request
- **THEN** `permissions_store::set` writes `1` for each individual bit (`MIC_STREAM`, `CAMERA_STREAM`) under the origin
- **AND** a later request for either bit individually returns `Decision::Allow`

### Requirement: Store module is decoupled from the handler
The shell SHALL expose the permission store as a standalone module `src/permissions_store.rs` with a stable API (`Decision`, `get`, `set`) so that M20 can substitute the GSettings backend with per-account JSON without modifying `src/handlers/permission.rs`.

#### Scenario: Handler only depends on the public API
- **WHEN** the permission handler resolves or records a decision
- **THEN** it calls `permissions_store::get` and `permissions_store::set` exclusively
- **AND** does not reference `gio::Settings` or the `permission-decisions` key directly

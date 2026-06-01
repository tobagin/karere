# account-manager Specification

## Purpose
TBD - created by archiving change m20-multi-account-discovery. Update Purpose after archive.
## Requirements
### Requirement: Account record shape
The system SHALL define an `Account` struct in `src/accounts.rs` that captures one WhatsApp Web account's identity, session state, and per-account preferences.

#### Scenario: Required fields are present
- **WHEN** the `Account` struct is compiled
- **THEN** it contains at minimum: `id: String` (UUID), `wid: Option<String>`, `pushname: Option<String>`, `user_label: Option<String>`, `avatar_png: Option<Vec<u8>>`, `avatar_url: Option<String>`, `created_at: i64`, `last_used_at: i64`, `is_active: bool`, `has_session: bool`, `has_unread: bool`, `zoom_level: f64`, and `permissions: AccountPermissions`

#### Scenario: Identity fields are auto-discovered, not user-edited
- **WHEN** the UI presents an account record for editing
- **THEN** only `user_label` is editable
- **AND** `wid`, `pushname`, `avatar_png`, and `avatar_url` are displayed read-only (greyed)

### Requirement: Atomic JSON persistence
The system SHALL persist all accounts to `$XDG_DATA_HOME/karere/accounts/accounts.json` using a temp-then-rename write strategy.

#### Scenario: Save writes to temp file then renames
- **WHEN** `AccountManager::save` is called
- **THEN** the new JSON is written to `accounts.json.tmp` in the same directory
- **AND** `fs::rename` promotes it to `accounts.json`
- **AND** the operation either fully succeeds or leaves the prior `accounts.json` untouched

#### Scenario: Load tolerates missing file
- **WHEN** `AccountManager::load` is called and `accounts.json` does not exist
- **THEN** an empty in-memory account list is returned without error

#### Scenario: Load tolerates malformed JSON by failing visibly
- **WHEN** `AccountManager::load` is called and `accounts.json` exists but cannot be parsed
- **THEN** an error is returned identifying the parse failure
- **AND** the file is NOT silently overwritten

### Requirement: MRU ordering
The system SHALL sort accounts strictly by `last_used_at` descending whenever the UI requests the account list.

#### Scenario: Sorted accessor returns MRU order
- **WHEN** `AccountManager::get_accounts_sorted` is called
- **THEN** the returned `Vec<Account>` is sorted by `last_used_at` descending (most recent first)

#### Scenario: Activate updates last_used_at and persists
- **WHEN** `AccountManager::activate(id)` is called
- **THEN** `last_used_at` for the matching account is set to the current Unix timestamp
- **AND** `save` is invoked
- **AND** an `accounts-changed` signal is emitted

#### Scenario: No order field is exposed
- **WHEN** the `Account` struct is compiled
- **THEN** it does NOT contain an `order` field or any user-controllable sort key

### Requirement: Account CRUD API
The system SHALL provide `add`, `remove`, `update_identity`, and `update_avatar` operations on `AccountManager`.

#### Scenario: Add returns a new Account with a UUID
- **WHEN** `AccountManager::add()` is called
- **THEN** a new `Account` is created with a freshly generated UUID `id`
- **AND** `created_at` and `last_used_at` are set to the current Unix timestamp
- **AND** the account is appended to the in-memory list and `save` is invoked

#### Scenario: Remove deletes by id and persists
- **WHEN** `AccountManager::remove(id)` is called for an existing account
- **THEN** the account is removed from the in-memory list
- **AND** `save` is invoked
- **AND** an `accounts-changed` signal is emitted

#### Scenario: Update identity stores wid and pushname
- **WHEN** `AccountManager::update_identity(id, wid, pushname)` is called
- **THEN** the matching account's `wid` and `pushname` fields are updated
- **AND** `save` is invoked

#### Scenario: Update avatar stores decoded PNG bytes
- **WHEN** `AccountManager::update_avatar(id, png_bytes)` is called
- **THEN** the matching account's `avatar_png` is set to the supplied bytes
- **AND** `save` is invoked


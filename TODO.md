# Multi-Account Completion Checklist

This branch (`feat/multi-account-completion`) contains the refactored WebView logic to support multiple accounts correctly.

## Completed

- [x] **Fix Notification Routing** — `setup_webview` captures `account_id` per-closure so notifications are always tied to their source account.
- [x] **Implement Unread Counts via Title** — `notify::title` handler parses WhatsApp Web page titles (e.g., `(3) WhatsApp`) and updates `AccountManager::set_account_unread`, tray icon, and account button.
- [x] **Account UI Polish** — Unused constants removed; emoji/color avatar system implemented; `DEFAULT_ACCOUNT_ID` constant extracted.
- [x] **Tray Refresh on Account CRUD** — `refresh-tray-accounts` action keeps tray menu in sync on add/edit/delete/switch.

## Remaining

### 4. Verify "Add Account" Flow & Persistence
- **Verification**: Manually verify that adding a second account:
    - Persists across restarts.
    - Correctly requests camera permissions.
    - Can be removed/logged out.

### ~~5. Translations~~ ✓
- All 6 locale files (es, ga, pt_BR, pt_PT, en_UK, en_US) cleaned up: fuzzy markers removed, empty strings filled, cross-language contamination fixed.

### 6. Snap Packaging
- **Verification**: Ensure `snap/snapcraft.yaml` builds a working snap package.

# Tasks

- [x] 1. Research WebKit 6.0 WebsiteDataManager API for persistent storage configuration
- [x] 2. Add WebsiteDataManager configuration to WebViewManager with proper storage paths
- [x] 3. Verify localStorage and IndexedDB are enabled in WebKitManager settings
- [x] 4. Add storage location logging for debugging
- [x] 5. Document storage paths in code comments
- [x] 6. Validate all changes with `openspec validate --strict`
- [x] 7. Build and test dev Flatpak with `./scripts/build.sh --dev`
- [x] 8. Verify localStorage is working (109 items found in database)
- [x] 9. Investigate WhatsApp Web notification banner behavior
- [x] 10. Implement Notification.permission injection to fix banner
- [x] 11. Fix notification handler setup to restore notifications
- [x] 12. Set webview-zoom-enabled default to false in gschema
- [x] 13. Test complete notification flow (VERIFIED WORKING)

## Final Solution

### Root Cause
WhatsApp Web checks `Notification.permission` on page load. Even though WebKit grants permission via `request.allow()`, it doesn't persist the permission state in a way that JavaScript queries can detect. This caused:
1. ❌ Banner showing "Message notifications are off. Turn on"
2. ❌ Notifications not working (handler wasn't being set up)

### The Fix
**File**: `src/managers/WebKitNotificationBridge.vala`

Added two-part solution:
1. **JavaScript Injection**: Inject `Notification.permission = 'granted'` at page start using `WebKit.UserScript`
   - Runs before WhatsApp Web loads
   - Makes WhatsApp Web think permission is already granted
   - Fixes the banner issue

2. **Handler Setup**: Call `setup_notification_handler()` to connect `show_notification` signal
   - Allows WebKit to send notifications to our bridge
   - Fixes notifications actually showing

**Additional Fix**: Set `webview-zoom-enabled` default to `false` in gschema.xml

## Test Results

✅ **Banner**: No longer appears after granting permission once
✅ **Notifications**: Working correctly when WhatsApp messages arrive
✅ **Persistence**: Both banner state and notifications persist across app restarts
✅ **Zoom**: Disabled by default for new installations

## Technical Details

**Storage Verification:**
- WebKitGTK 6.0 uses persistent storage by default: `~/.var/app/io.github.tobagin.karere.Devel/data/Karere (Devel)/storage/`
- localStorage.sqlite3 confirmed working (109+ items)
- IndexedDB present and functional
- Cookies persisting correctly (user stays logged in)

**Why the Injection Works:**
- WhatsApp Web queries `Notification.permission` via JavaScript
- WebKit's permission grant via `request.allow()` is internal only
- JavaScript injection bridges this gap by making the permission visible to web code
- Handler connection ensures actual notifications flow through WebKit's bridge

# Proposal: Fix WhatsApp Notification Preference Persistence

## Why

WhatsApp Web displays a persistent "Message notifications are off. Turn on" banner every time the user opens the app, even though:
1. The native notification permission has been granted at the system level (handled by `WebKitNotificationBridge`)
2. The user has clicked "Turn on" within WhatsApp Web in previous sessions

**Root Cause**: WhatsApp Web stores its own internal notification preference (separate from the browser permission) in browser storage (likely localStorage or IndexedDB). Currently, Karere's WebKit storage configuration may not be properly persisting this data between sessions, causing WhatsApp Web to "forget" the user's notification preference.

**Impact**: This creates a poor user experience with a persistent nag banner that requires the user to re-enable notifications within WhatsApp Web on every app launch, despite having already granted permission.

## What Changes

This change will ensure WebKit properly persists website data (localStorage, IndexedDB, cookies) for WhatsApp Web so that the notification preference setting is maintained across sessions.

### Specifications Affected

- **webview-manager** (MODIFIED) - Add website data manager configuration to properly persist web storage
- **webkit-storage** (ADDED) - New spec for WebKit storage persistence and management

### Key Changes

1. Configure WebKit's website data manager to use persistent storage paths
2. Ensure localStorage and IndexedDB are enabled and properly persisted
3. Configure appropriate storage paths within the Flatpak sandbox
4. Add storage cleanup capabilities for privacy

### Implementation Approach

- Use `WebKit.WebsiteDataManager` to configure persistent storage locations
- Set base data directory to `~/.var/app/io.github.tobagin.karere.Devel/data/webkitgtk/`
- Enable localStorage, IndexedDB, and offline web application cache persistence
- Ensure storage survives app restarts and system reboots

### Testing Strategy

1. Launch Karere and grant notification permission (both system and WhatsApp Web)
2. Close and reopen Karere
3. Verify WhatsApp Web does NOT show the "Message notifications are off" banner
4. Verify notifications still work after restart
5. Test storage cleanup functionality

# Refactor Window.vala into Smaller Components

## Why
Window.vala has grown to 1196 lines with 9 distinct responsibilities (WebView management, clipboard operations, notifications, accessibility, window state, etc.), violating the Single Responsibility Principle. This makes the code difficult to maintain, hard to test independently, and prone to merge conflicts. The existing codebase already demonstrates the manager pattern successfully with WebKitManager, NotificationManager, and AccessibilityManager, so Window.vala should follow this established pattern.

## What Changes
- Extract WebView lifecycle, navigation policy, and external link handling into `WebViewManager` (~400 lines)
- Extract clipboard image paste detection, processing, and WhatsApp injection into `ClipboardManager` (~250 lines)
- Extract window geometry persistence into `WindowStateManager` (~80 lines)
- Extract WebKit notification permission and bridging into `WebKitNotificationBridge` (~150 lines)
- Reduce Window.vala from 1196 lines to ~400 lines focused on UI composition and manager coordination
- Add unit tests for all new manager classes

## Impact
- **Affected specs**: Creates 4 new capabilities (webview-manager, clipboard-manager, window-state-manager, webkit-notification-bridge)
- **Affected code**: `src/Window.vala` (major refactoring), `src/managers/` (4 new files), `tests/` (4 new test files), `meson.build` (add new sources and tests)
- **Breaking changes**: None - public API preserved
- **Migration**: None required - internal refactoring only

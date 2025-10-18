# Tasks: Refactor Window.vala Components

## Overview
This task list breaks down the Window.vala refactoring into small, verifiable work items that deliver incremental progress. The tasks are ordered to minimize risk and enable parallel work where possible.

## Task Sequence

### Phase 1: Preparation & Foundation

#### Task 1.1: Create WindowStateManager skeleton
- Create `src/managers/WindowStateManager.vala` with GPL header
- Define class structure with constructor accepting Settings and Window
- Add empty methods: `restore_state()`, `start_tracking()`, `save_state()`
- Add to `meson.build` sources list
- Verify compilation

**Validation**: Build succeeds with no warnings

**Estimated effort**: 15 minutes

---

#### Task 1.2: Implement WindowStateManager state restoration
- Implement `restore_state()` method
- Read window-width, window-height, window-maximized from GSettings
- Apply values to window using `set_default_size()` and `maximize()`
- Add null settings handling with fallback to 1200x800 defaults
- Add debug logging

**Validation**: Manual test - verify window restores saved size/maximized state on startup

**Estimated effort**: 30 minutes

---

#### Task 1.3: Implement WindowStateManager state tracking
- Implement `start_tracking()` method
- Connect to window notify["maximized"], notify["default-width"], notify["default-height"] signals
- Implement `save_state()` method
- Read current window state and write to GSettings
- Add null settings handling
- Add debug logging

**Validation**: Manual test - verify window size/maximize changes are persisted and restored

**Estimated effort**: 30 minutes

---

#### Task 1.4: Integrate WindowStateManager into Window.vala
- Instantiate WindowStateManager in Window constructor
- Call `restore_state()` after all setup
- Call `start_tracking()` after restore
- Call `save_state()` in `close_request()`
- Remove old `restore_window_state()` and `save_window_state()` methods
- Remove direct signal connections for state tracking

**Validation**: Full integration test - window state persistence still works; build succeeds

**Estimated effort**: 20 minutes

---

### Phase 2: Clipboard Manager

#### Task 2.1: Create ClipboardManager skeleton
- Create `src/managers/ClipboardManager.vala` with GPL header
- Define class with constructor accepting Gdk.Clipboard and WebKit.WebView
- Add signals: `paste_started()`, `paste_succeeded(string)`, `paste_failed(string)`
- Add empty methods: `setup_paste_detection()`, `handle_paste_request()`
- Add to `meson.build` sources list
- Verify compilation

**Validation**: Build succeeds with no warnings

**Estimated effort**: 15 minutes

---

#### Task 2.2: Implement paste event detection
- Implement `setup_paste_detection()` to create EventControllerKey
- Add controller to WebView widget
- Implement `on_key_pressed()` to detect Ctrl+V
- Call `handle_paste_request()` when Ctrl+V detected
- Add debug logging

**Validation**: Unit test - verify Ctrl+V is detected and other keys pass through

**Estimated effort**: 30 minutes

---

#### Task 2.3: Implement clipboard image format detection
- Implement `handle_paste_request()` to query clipboard formats
- Detect Gdk.Texture type
- Detect image/png, image/jpeg, image/gif MIME types
- Call appropriate processing method for each format
- Emit `paste_started` signal
- Add null clipboard handling

**Validation**: Unit test - verify different image formats are detected correctly

**Estimated effort**: 45 minutes

---

#### Task 2.4: Implement image processing and conversion
- Implement `process_clipboard_image()` for Gdk.Texture
- Convert texture to PNG bytes
- Implement `process_clipboard_image_stream()` for MIME streams
- Read stream into MemoryOutputStream
- Convert both to base64 data URLs
- Add error handling and logging

**Validation**: Unit test - verify image conversion produces valid base64 data URLs

**Estimated effort**: 45 minutes

---

#### Task 2.5: Implement WhatsApp image injection
- Implement `inject_image_into_whatsapp()` method
- Create JavaScript to find WhatsApp message box
- Create File object from data URL
- Create and dispatch ClipboardEvent
- Execute JavaScript in WebView
- Emit `paste_succeeded` or `paste_failed` signals
- Add error handling and logging

**Validation**: Manual test - paste image from clipboard into WhatsApp Web

**Estimated effort**: 1 hour

---

#### Task 2.6: Implement default paste fallback
- Implement `inject_default_paste()` method
- Create JavaScript to execute document.execCommand('paste')
- Handle non-image clipboard content
- Add debug logging

**Validation**: Manual test - paste text into WhatsApp Web works normally

**Estimated effort**: 20 minutes

---

#### Task 2.7: Integrate ClipboardManager into Window.vala
- Instantiate ClipboardManager in Window constructor
- Call `setup_paste_detection()` with web_view
- Connect to paste_succeeded/paste_failed signals for toasts
- Remove old `setup_clipboard_paste()` and related methods (7 methods total)
- Remove old EventControllerKey code

**Validation**: Full integration test - clipboard paste still works; build succeeds; ~220 lines removed from Window.vala

**Estimated effort**: 30 minutes

---

### Phase 3: WebKit Notification Bridge

#### Task 3.1: Create WebKitNotificationBridge skeleton
- Create `src/managers/WebKitNotificationBridge.vala` with GPL header
- Define class with constructor accepting Settings, NotificationManager, parent window
- Add empty methods: `setup()`, `on_permission_request()`, `show_notification_permission_dialog()`
- Add to `meson.build` sources list
- Verify compilation

**Validation**: Build succeeds with no warnings

**Estimated effort**: 15 minutes

---

#### Task 3.2: Implement permission persistence check
- Implement `on_permission_request()` to check for WebKit.NotificationPermissionRequest
- Read web-notification-permission-asked and web-notification-permission-granted from GSettings
- If previously asked, apply saved decision (allow or deny)
- If not asked, call `show_notification_permission_dialog()`
- Add null settings handling
- Add logging

**Validation**: Unit test - verify saved permissions are correctly applied

**Estimated effort**: 45 minutes

---

#### Task 3.3: Implement permission dialog
- Implement `show_notification_permission_dialog()` using Adw.AlertDialog
- Add "Deny" and "Allow" responses
- Set "Allow" as suggested and default response
- Connect response signal to save decision and handle permission
- Present dialog on parent window
- Add logging

**Validation**: Manual test - permission dialog appears and saves decision

**Estimated effort**: 30 minutes

---

#### Task 3.4: Implement notification handler setup
- Implement `setup_notification_handler()` method
- Connect to WebView show_notification signal
- Add logging

**Validation**: Unit test - verify signal connection is established

**Estimated effort**: 15 minutes

---

#### Task 3.5: Implement WebKit to native notification bridging
- Implement `on_webkit_notification()` method
- Extract title and body from WebKit.Notification
- Call NotificationManager.send_notification()
- Connect to notification clicked signal to present window
- Connect to notification closed signal for logging
- Close WebKit notification after creating native one
- Add null notification_manager handling

**Validation**: Manual test - WebKit notification creates native notification; clicking focuses window

**Estimated effort**: 45 minutes

---

#### Task 3.6: Integrate WebKitNotificationBridge into Window.vala
- Instantiate WebKitNotificationBridge in Window constructor
- Call `setup()` with web_view in `setup_notifications()`
- Remove old permission request handling code
- Remove old `on_permission_request()`, `show_notification_permission_dialog()`, `setup_notification_handler()`, `on_webkit_notification()` methods
- Remove WebKit notification-related signal connections

**Validation**: Full integration test - notification permission dialog still works; notifications display; build succeeds; ~130 lines removed from Window.vala

**Estimated effort**: 30 minutes

---

### Phase 4: WebView Manager

#### Task 4.1: Create WebViewManager skeleton
- Create `src/managers/WebViewManager.vala` with GPL header
- Define class with constructor accepting Settings and WebKitManager
- Add signals: `load_started()`, `load_finished()`, `load_failed(string, string)`, `external_link_clicked(string)`
- Add property: `public WebKit.WebView web_view { get; private set; }`
- Add empty methods: `setup()`, `reload()`, `open_developer_tools()`, etc.
- Add to `meson.build` sources list
- Verify compilation

**Validation**: Build succeeds with no warnings

**Estimated effort**: 20 minutes

---

#### Task 4.2: Implement WebView creation and setup
- Implement WebView creation in constructor
- Implement `setup()` method to configure and add to container
- Use WebKitManager to configure WebView settings
- Load https://web.whatsapp.com
- Add null settings handling
- Add debug logging

**Validation**: Manual test - WebView loads WhatsApp Web

**Estimated effort**: 30 minutes

---

#### Task 4.3: Implement URL classification methods
- Implement `is_whatsapp_internal_uri()` method
- Check for web.whatsapp.com, static.whatsapp.net, wss://, blob:, data:, about: URIs
- Implement `is_external_link()` method
- Check for HTTP/HTTPS not matching internal patterns

**Validation**: Unit test - verify correct classification of various URLs

**Estimated effort**: 30 minutes

---

#### Task 4.4: Implement navigation policy handling
- Implement `on_navigation_policy_decision()` method
- Use URL classification methods
- Allow internal navigation
- Emit `external_link_clicked` signal for external links
- Open external links via portal
- Implement `on_create_new_web_view()` for new window requests
- Add logging

**Validation**: Manual test - internal links work; external links open in browser

**Estimated effort**: 1 hour

---

#### Task 4.5: Implement external link opening
- Implement `open_uri_external()` method
- Implement `open_uri_with_portal()` using AppInfo.launch_default_for_uri_async
- Add error handling
- Add logging

**Validation**: Manual test - external links open in system browser

**Estimated effort**: 20 minutes

---

#### Task 4.6: Implement load event handling
- Implement `on_load_changed()` method
- Emit appropriate signals (load_started, load_finished)
- Coordinate with WebKitManager for user agent injection on load finished
- Implement `on_load_failed()` method
- Emit load_failed signal with details
- Add logging

**Validation**: Manual test - load events fire correctly; user agent injection still works

**Estimated effort**: 30 minutes

---

#### Task 4.7: Implement developer tools management
- Implement `open_developer_tools()` method with settings check
- Implement `is_developer_tools_open()` method
- Implement `close_developer_tools()` method
- Implement `update_developer_tools_setting()` method
- Add logging

**Validation**: Manual test - developer tools open/close/check work correctly

**Estimated effort**: 30 minutes

---

#### Task 4.8: Implement reload and JavaScript injection
- Implement `reload()` method with force parameter
- Use `web_view.reload()` or `web_view.reload_bypass_cache()`
- Implement `inject_javascript()` helper method
- Wrap WebView.evaluate_javascript for cleaner API
- Add logging

**Validation**: Manual test - reload works; JavaScript injection works

**Estimated effort**: 20 minutes

---

#### Task 4.9: Integrate WebViewManager into Window.vala
- Instantiate WebViewManager in Window constructor (pass settings and webkit_manager)
- Call `setup()` with web_container in `setup_webkit()`
- Connect to all WebViewManager signals
- Update public methods to delegate to WebViewManager
- Remove old WebView creation and configuration code
- Remove old navigation policy methods (6 methods)
- Remove old load event handlers
- Remove direct WebView setup code
- Update `web_view` references to use `webview_manager.web_view`

**Validation**: Full integration test - WebView still works; navigation works; developer tools work; build succeeds; ~400 lines removed from Window.vala

**Estimated effort**: 1.5 hours

---

### Phase 5: Testing & Cleanup

#### Task 5.1: Write WindowStateManager unit tests
- Create `tests/test_window_state_manager.vala`
- Test state restoration with valid settings
- Test state restoration with null settings (fallback)
- Test state tracking and persistence
- Test signal connections
- Add to meson.build test sources

**Validation**: All tests pass; meson test succeeds

**Estimated effort**: 1 hour

---

#### Task 5.2: Write ClipboardManager unit tests
- Create `tests/test_clipboard_manager.vala`
- Test paste detection (Ctrl+V vs other keys)
- Test image format detection (mock clipboard)
- Test image conversion (texture to base64)
- Test signal emissions
- Add to meson.build test sources

**Validation**: All tests pass; meson test succeeds

**Estimated effort**: 1.5 hours

---

#### Task 5.3: Write WebKitNotificationBridge unit tests
- Create `tests/test_webkit_notification_bridge.vala`
- Test permission persistence check
- Test permission dialog creation
- Test WebKit notification handling (mock WebKit.Notification)
- Test signal connections
- Add to meson.build test sources

**Validation**: All tests pass; meson test succeeds

**Estimated effort**: 1.5 hours

---

#### Task 5.4: Write WebViewManager unit tests
- Create `tests/test_webview_manager.vala`
- Test URL classification methods
- Test navigation policy (mock PolicyDecision)
- Test load event handling
- Test developer tools methods
- Test reload functionality
- Add to meson.build test sources

**Validation**: All tests pass; meson test succeeds

**Estimated effort**: 2 hours

---

#### Task 5.5: Update Window.vala tests
- Update `tests/test_window.vala`
- Test Window constructor with new managers
- Test public API methods still work
- Test manager integration
- Verify Window.vala line count is ~400

**Validation**: All tests pass; Window.vala reduced to target size

**Estimated effort**: 1 hour

---

#### Task 5.6: Manual integration testing
- Test full application workflow
- Verify window state persistence
- Verify clipboard image paste
- Verify text paste fallback
- Verify notification permission dialog
- Verify native notifications
- Verify external link opening
- Verify developer tools
- Verify WebView reload
- Check for any regressions

**Validation**: All functionality works as before; no regressions

**Estimated effort**: 1 hour

---

#### Task 5.7: Code cleanup and documentation
- Remove any unused imports from Window.vala
- Add/update class-level documentation comments for new managers
- Ensure all methods have descriptive comments
- Verify GPL headers on all new files
- Format code per project conventions (4-space indentation)

**Validation**: Code review; no linting warnings

**Estimated effort**: 30 minutes

---

#### Task 5.8: Build verification
- Run full clean build: `meson setup build --wipe && ninja -C build`
- Run all tests: `meson test -C build`
- Verify no new compiler warnings
- Test Flatpak build: `./scripts/build.sh`
- Run Flatpak to verify functionality

**Validation**: Clean build; all tests pass; Flatpak runs correctly

**Estimated effort**: 30 minutes

---

## Summary

**Total tasks**: 37 tasks across 5 phases

**Estimated total effort**: ~17 hours

**Parallelizable work**:
- Phase 2 (ClipboardManager) and Phase 3 (WebKitNotificationBridge) can be worked in parallel after Phase 1
- Test writing (5.1-5.4) can be partially parallel

**Critical path**:
Phase 1 → Phase 4 → Phase 5 (WindowStateManager → WebViewManager → Testing)

**Risk mitigation**:
- Each phase delivers working, testable code
- Small, focused tasks reduce error introduction
- Manual testing after each integration catches issues early
- Unit tests provide regression safety net

**Dependencies**:
- All phases depend on Phase 1 completion (foundation)
- Phase 4 (WebViewManager) depends on existing WebKitManager
- Phase 3 (WebKitNotificationBridge) depends on existing NotificationManager
- Phase 5 (Testing) depends on all implementation phases

**Success criteria**:
✅ Window.vala reduced from 1196 lines to ~400 lines (66% reduction)
✅ 4 new manager classes created and tested
✅ All existing tests pass
✅ New unit tests achieve 80%+ coverage
✅ Zero behavior changes (manual verification)
✅ Clean build with no new warnings

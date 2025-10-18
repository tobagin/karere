# Design: Window.vala Component Refactoring

## Overview
This design documents the architectural approach for extracting Window.vala's responsibilities into specialized manager classes following the established manager pattern in the Karere codebase.

## Current Architecture

### Window.vala Responsibilities (1196 lines)
```
Window.vala
├── UI Setup & Lifecycle (~70 lines)
│   ├── setup_window_properties()
│   ├── setup_actions()
│   └── Constructor orchestration
├── WebView Management (~400 lines)
│   ├── setup_webkit()
│   ├── configure_cookie_storage()
│   ├── setup_spell_checking()
│   └── Spell checking language detection
├── Navigation & Policy (~150 lines)
│   ├── on_navigation_policy_decision()
│   ├── on_create_new_web_view()
│   ├── is_whatsapp_internal_uri()
│   ├── is_external_link()
│   ├── open_uri_external()
│   └── open_uri_with_portal()
├── WebView Lifecycle Events (~50 lines)
│   ├── on_load_changed()
│   └── on_load_failed()
├── Clipboard Operations (~220 lines)
│   ├── setup_clipboard_paste()
│   ├── on_key_pressed() [Ctrl+V detection]
│   ├── handle_paste_event()
│   ├── process_clipboard_image()
│   ├── process_clipboard_image_stream()
│   ├── inject_image_into_whatsapp()
│   └── inject_default_paste()
├── Notifications (~130 lines)
│   ├── setup_notifications()
│   ├── setup_webkit_notifications()
│   ├── on_permission_request()
│   ├── show_notification_permission_dialog()
│   ├── setup_notification_handler()
│   └── on_webkit_notification()
├── Accessibility (~80 lines)
│   ├── setup_accessibility_features()
│   ├── setup_aria_roles()
│   ├── setup_skip_links()
│   ├── setup_focus_management()
│   ├── on_window_focus_in()
│   ├── on_window_focus_out()
│   └── update_focus_indicators()
├── Settings Listeners (~30 lines)
│   └── setup_settings_listeners() [7 signal connections]
├── Window State (~50 lines)
│   ├── restore_window_state()
│   └── save_window_state()
├── Developer Tools (~40 lines)
│   ├── open_developer_tools()
│   ├── is_developer_tools_open()
│   ├── close_developer_tools()
│   └── update_developer_tools()
├── Zoom Controls (~70 lines)
│   ├── webkit_zoom_in()
│   ├── webkit_zoom_out()
│   ├── webkit_zoom_reset()
│   ├── update_webkit_zoom()
│   └── update_zoom_controls_visibility()
├── Toast Messages (~30 lines)
│   ├── show_error_toast()
│   ├── show_info_toast()
│   ├── show_success_toast()
│   └── show_toast()
└── Lifecycle Management (~20 lines)
    ├── close_request()
    └── dispose()
```

### Dependencies
```
Window.vala currently depends on:
├── Gtk.Application (parent)
├── Settings (GSettings)
├── WebKitManager (already extracted)
├── NotificationManager (already extracted)
├── AccessibilityManager (via Application)
├── KeyboardShortcuts (via Application)
└── WebKit.WebView (direct ownership)
```

## Proposed Architecture

### New Structure
```
Window.vala (~400 lines)
├── Manager instantiation and coordination
├── UI composition (header_bar, toast_overlay, web_container)
├── GTK template child bindings
├── Public API methods (show_toast, reload_webview, etc.)
├── Lifecycle (constructor, close_request, dispose)
└── Minimal glue code between managers

WebViewManager (~400 lines) [NEW]
├── WebView lifecycle management
├── Navigation policy decisions
├── External link handling
├── Load event handling
├── WebView configuration coordination
└── Developer tools management

ClipboardManager (~250 lines) [NEW]
├── Clipboard paste detection (Ctrl+V)
├── Image format detection (texture, PNG, JPEG, GIF)
├── Image processing and conversion
├── WhatsApp injection via JavaScript
└── Default paste fallback

WindowStateManager (~80 lines) [NEW]
├── Window size/position persistence to GSettings
├── Maximize state tracking
├── State restoration on startup
└── Signal connection for state changes

WebKitNotificationBridge (~150 lines) [NEW]
├── WebKit permission request handling
├── Permission persistence to GSettings
├── Permission dialog presentation
├── WebKit.Notification to native notification bridging
└── Notification click/close event handling
```

## Component Design

### 1. WebViewManager

**Purpose**: Manage WebView lifecycle, navigation, and policy decisions.

**Location**: `src/managers/WebViewManager.vala`

**Public API**:
```vala
public class WebViewManager : Object {
    // Signals
    public signal void load_started();
    public signal void load_finished();
    public signal void load_failed(string uri, string error_message);
    public signal void external_link_clicked(string uri);

    // Properties
    public WebKit.WebView web_view { get; private set; }

    // Constructor
    public WebViewManager(Settings? settings, WebKitManager webkit_manager);

    // Methods
    public void setup(Gtk.Box container);
    public void reload(bool force = false);
    public bool is_developer_tools_open();
    public void open_developer_tools();
    public void close_developer_tools();
    public void update_developer_tools_setting(bool enabled);
    public void inject_javascript(string script, owned JSCore.CompletionHandler? callback = null);
}
```

**Key Responsibilities**:
- Create and configure WebView widget
- Handle navigation policy (internal vs external URLs)
- Manage load events (started, finished, failed)
- Coordinate with WebKitManager for settings
- Handle developer tools lifecycle
- Provide JavaScript injection abstraction

**Integration Points**:
- Receives WebKitManager in constructor for configuration
- Emits signals for events Window needs to react to
- Manages WebView widget and adds to container provided by Window

### 2. ClipboardManager

**Purpose**: Handle clipboard image paste operations and WhatsApp injection.

**Location**: `src/managers/ClipboardManager.vala`

**Public API**:
```vala
public class ClipboardManager : Object {
    // Signals
    public signal void paste_started();
    public signal void paste_succeeded(string image_type);
    public signal void paste_failed(string error_message);

    // Constructor
    public ClipboardManager(Gdk.Clipboard clipboard, WebKit.WebView web_view);

    // Methods
    public void setup_paste_detection(Gtk.Widget widget);
    public void handle_paste_request();
}
```

**Key Responsibilities**:
- Detect Ctrl+V keypresses via EventControllerKey
- Query clipboard for image formats (Texture, PNG, JPEG, GIF)
- Convert clipboard data to bytes/base64
- Inject images into WhatsApp via JavaScript
- Fallback to default paste for non-image content

**Integration Points**:
- Receives WebView reference for JavaScript execution
- Receives Clipboard reference from Window
- Emits signals for paste success/failure (for toast messages)
- Adds EventControllerKey to WebView widget

### 3. WindowStateManager

**Purpose**: Persist and restore window geometry and state.

**Location**: `src/managers/WindowStateManager.vala`

**Public API**:
```vala
public class WindowStateManager : Object {
    // Constructor
    public WindowStateManager(Settings? settings, Adw.ApplicationWindow window);

    // Methods
    public void restore_state();
    public void start_tracking();
    public void save_state();
}
```

**Key Responsibilities**:
- Read window size/position/maximized from GSettings
- Apply state to window on startup
- Listen to window property changes (notify signals)
- Save state changes to GSettings
- Handle null Settings gracefully (fallback to defaults)

**Integration Points**:
- Receives Settings and Window references
- Connects to window notify signals for tracking
- Called from Window constructor and close_request

### 4. WebKitNotificationBridge

**Purpose**: Bridge WebKit notification permission and events to native notifications.

**Location**: `src/managers/WebKitNotificationBridge.vala`

**Public API**:
```vala
public class WebKitNotificationBridge : Object {
    // Constructor
    public WebKitNotificationBridge(
        Settings? settings,
        NotificationManager notification_manager,
        Adw.ApplicationWindow parent_window
    );

    // Methods
    public void setup(WebKit.WebView web_view);
}
```

**Key Responsibilities**:
- Handle WebKit.PermissionRequest for notifications
- Show native permission dialog using Adw.AlertDialog
- Persist permission decision to GSettings
- Convert WebKit.Notification to native notification via NotificationManager
- Handle notification click events (focus window)

**Integration Points**:
- Receives NotificationManager for sending native notifications
- Receives parent window for presenting permission dialog
- Connects to WebView permission_request and show_notification signals
- Uses Settings for permission persistence

## Migration Strategy

### Phase 1: Extract WindowStateManager
- **Rationale**: Smallest, most isolated component (~80 lines)
- **Risk**: Low - simple GSettings read/write
- **Testing**: Verify window size/position persistence works

### Phase 2: Extract ClipboardManager
- **Rationale**: Self-contained, no dependencies on other new managers
- **Risk**: Medium - complex JavaScript injection logic
- **Testing**: Test image paste, fallback paste, error cases

### Phase 3: Extract WebKitNotificationBridge
- **Rationale**: Moderately complex, depends on NotificationManager (already exists)
- **Risk**: Medium - permission dialog and event handling
- **Testing**: Test permission grant/deny, notification display, click handling

### Phase 4: Extract WebViewManager
- **Rationale**: Largest component, should be last
- **Risk**: Medium-High - central to application, many integration points
- **Testing**: Test navigation, external links, load events, developer tools

### Phase 5: Clean up Window.vala
- **Rationale**: Remove extracted code, simplify constructor
- **Risk**: Low - just cleanup
- **Testing**: Full integration test, ensure no regressions

## Code Organization

### Directory Structure
```
src/
├── managers/
│   ├── WebViewManager.vala          [NEW - 400 lines]
│   ├── ClipboardManager.vala        [NEW - 250 lines]
│   ├── WindowStateManager.vala      [NEW - 80 lines]
│   ├── WebKitNotificationBridge.vala [NEW - 150 lines]
│   ├── WebKitManager.vala           [EXISTING]
│   ├── NotificationManager.vala     [EXISTING]
│   ├── AccessibilityManager.vala    [EXISTING]
│   ├── SettingsManager.vala         [EXISTING]
│   └── ...
└── Window.vala                       [MODIFIED - 1196→400 lines]
```

### Naming Conventions
- All managers follow PascalCase naming
- All managers extend GLib.Object
- All managers in `Karere` namespace
- All managers include GPL-3.0-or-later header
- Method names use snake_case per project conventions

## Testing Strategy

### Unit Tests
```
tests/
├── test_webview_manager.vala        [NEW]
├── test_clipboard_manager.vala      [NEW]
├── test_window_state_manager.vala   [NEW]
├── test_webkit_notification_bridge.vala [NEW]
└── test_window.vala                 [UPDATED]
```

### Test Coverage Goals
- **WebViewManager**: Navigation policy, external link detection, load events
- **ClipboardManager**: Image detection, base64 conversion, JavaScript injection
- **WindowStateManager**: State save/restore, null settings handling
- **WebKitNotificationBridge**: Permission dialog, notification bridging
- **Window**: Manager coordination, lifecycle, public API

### Integration Testing
- Manual testing of window resize/maximize/restore
- Manual testing of image paste to WhatsApp
- Manual testing of notification permission and display
- Manual testing of external link opening
- Build verification (no warnings/errors)

## Backward Compatibility

### Public API Preservation
Window.vala public methods remain unchanged:
- `show_error_toast()`
- `show_info_toast()`
- `show_success_toast()`
- `show_toast()`
- `reload_webview()`
- `open_developer_tools()`
- `is_developer_tools_open()`
- `close_developer_tools()`
- `update_webkit_zoom()`
- `webkit_zoom_in()`
- `webkit_zoom_out()`
- `webkit_zoom_reset()`
- `update_focus_indicators()`
- `show_accessibility_status()`

### No Breaking Changes
- All existing behavior preserved
- All signals preserved
- All widget references preserved
- Application.vala integration unchanged

## Performance Considerations

### Minimal Overhead
- Manager instantiation happens once at window creation
- No additional signal hops for critical paths
- Direct method calls to managers (no indirection)
- No new memory allocations in hot paths

### Memory Impact
- 4 additional GLib.Object instances (~1KB total)
- Reduced Window.vala size improves code locality
- No changes to WebView or other heavy objects

## Error Handling

### Null Safety
All managers handle null Settings gracefully:
```vala
if (settings == null) {
    warning("Settings unavailable, using defaults");
    // Use fallback behavior
    return;
}
```

### Signal-based Error Propagation
Managers emit signals for errors Window can handle:
```vala
// ClipboardManager
public signal void paste_failed(string error_message);

// Window responds with toast
clipboard_manager.paste_failed.connect((error) => {
    show_error_toast(error);
});
```

### Logging
All managers use GLib logging:
- `debug()` for normal operations
- `info()` for important state changes
- `warning()` for recoverable errors
- `critical()` for serious errors

## Alternative Approaches Considered

### Alternative 1: Single "WindowHelper" Class
**Rejected**: Would still be too large (800+ lines) and not follow SRP

### Alternative 2: Extract Everything Including Toast/Zoom
**Rejected**: Over-engineering; toast methods are simple enough to stay in Window

### Alternative 3: Create ViewControllers Instead of Managers
**Rejected**: Would deviate from established "Manager" pattern in codebase

### Alternative 4: Use Composition with Interfaces
**Rejected**: Adds unnecessary complexity for a Vala/GTK application; direct manager references are clearer

## Open Questions & Decisions

### Q1: Should WebViewManager handle zoom controls?
**Answer**: Yes - zoom is WebView-specific and fits naturally in WebViewManager

**Rationale**: Zoom methods (`webkit_zoom_in()`, etc.) directly manipulate `web_view.zoom_level`, making them WebView operations. However, Window.vala will keep thin public API wrappers for backward compatibility.

### Q2: Where should spell checking configuration live?
**Answer**: Keep in WebViewManager initially; consider moving to WebKitManager later

**Rationale**: Spell checking spans both WebView setup (in WebViewManager) and WebKit settings (in WebKitManager). For this refactoring, keep it with WebView setup to minimize scope. A future change could consolidate all WebKit settings.

### Q3: Should we extract accessibility setup from Window.vala?
**Answer**: No - keep in Window.vala for now

**Rationale**: Window.vala's accessibility setup (~80 lines) is primarily about ARIA labels on Window's own widgets (header_bar, menu_button, web_container). This is window-specific UI setup, not a separable concern. The actual accessibility manager (AccessibilityManager) is already extracted.

### Q4: How to handle Settings dependency across all managers?
**Answer**: Pass Settings in each manager constructor; handle null gracefully

**Rationale**: Follows existing pattern (SettingsManager, WebKitManager). Each manager can independently decide on fallback behavior when settings unavailable.

## Success Criteria

### Quantitative Metrics
✅ Window.vala reduced from 1196 lines to ≤400 lines (66% reduction)
✅ 4 new manager classes created
✅ 0 new compiler warnings
✅ 0 behavior changes (regression testing)
✅ 100% of existing tests pass
✅ 80%+ code coverage for new manager tests

### Qualitative Metrics
✅ Improved code readability (measured by PR review feedback)
✅ Clearer separation of concerns
✅ Easier to test individual components
✅ Follows established codebase patterns
✅ Reduced cognitive load for understanding Window.vala

# Design: Add Download Manager

## Architecture Overview

This change introduces download management capabilities by adding a new `DownloadManager` class and enhancing existing components to support custom download directories and download notifications.

### Component Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                        Application                          │
│  - Creates DownloadManager singleton                        │
│  - Passes to Window on creation                             │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                          Window                             │
│  - Receives DownloadManager reference                       │
│  - Passes to WebViewManager                                 │
│  - Displays download toast notifications                    │
└─────────────────────────────────────────────────────────────┘
                              │
              ┌───────────────┴───────────────┐
              ▼                               ▼
┌──────────────────────────┐    ┌──────────────────────────┐
│    WebViewManager        │    │   DownloadManager        │
│  - Handles policy        │───▶│  - Detects downloads     │
│    decisions             │    │  - Manages directory     │
│  - Emits download signal │    │  - Opens files          │
└──────────────────────────┘    └──────────────────────────┘
                                            │
                                            ▼
                                ┌──────────────────────────┐
                                │   PreferencesDialog      │
                                │  - Download settings UI  │
                                │  - Portal folder picker  │
                                └──────────────────────────┘
```

## Key Design Decisions

### Decision 1: Dedicated DownloadManager Class

**Choice**: Create a new manager class instead of extending WebViewManager

**Rationale**:
- Follows existing manager pattern (NotificationManager, WebKitManager, etc.)
- Separation of concerns: WebViewManager handles WebView lifecycle, DownloadManager handles downloads
- Easier to test and maintain independently
- Allows future expansion (download history, progress tracking)

**Trade-offs**:
- Additional class increases codebase size (acceptable given benefits)
- Requires dependency injection (already established pattern)

### Decision 2: Download Detection via Policy Decisions

**Choice**: Use WebKit.PolicyDecisionType.RESPONSE in `decide_policy` signal

**Technical Details**:
```vala
web_view.decide_policy.connect((decision, decision_type) => {
    if (decision_type == WebKit.PolicyDecisionType.RESPONSE) {
        var response_decision = decision as WebKit.ResponsePolicyDecision;
        var response = response_decision.get_response();

        if (response.is_attachment || should_download(response)) {
            // Download detected
            download_manager.on_download_started(response.get_uri());
            decision.download();
            return true;
        }
    }
    decision.use();
    return false;
});
```

**Rationale**:
- Most reliable method in WebKitGTK 6.0
- Catches all download types (attachments, media, documents)
- Integrates cleanly with existing navigation policy handling
- Allows custom logic for determining what should download

**Alternatives Considered**:
- File system monitoring: Less reliable, higher overhead
- WebKit.Download signals: Requires more complex state tracking

### Decision 3: Toast Notifications with Action Buttons

**Choice**: Use AdwToast with action button instead of AdwBanner

**Implementation**:
```vala
public void show_download_toast(string filename, string file_path) {
    var message = _("Downloaded: %s").printf(filename);
    var toast = new Adw.Toast(message);
    toast.timeout = 5;
    toast.button_label = _("Open");
    toast.action_name = "app.open-downloaded-file";

    // Store file path for action
    var action = new SimpleAction("open-downloaded-file", null);
    action.activate.connect(() => {
        download_manager.open_file(file_path);
    });

    toast_overlay.add_toast(toast);
}
```

**Rationale**:
- Consistent with existing notification pattern (clipboard paste toasts)
- Non-intrusive, auto-dismissing
- Action button provides quick access without cluttering UI
- Follows GNOME HIG for transient notifications

### Decision 4: File Chooser Portal Integration

**Choice**: Use Gtk.FileDialog for portal integration (GTK 4.10+)

**Implementation**:
```vala
private async void select_download_directory() {
    var dialog = new Gtk.FileDialog();
    dialog.title = _("Select Download Directory");
    dialog.modal = true;

    try {
        var file = yield dialog.select_folder(preferences_window, null);
        var path = file.get_path();

        settings.set_string("custom-download-directory", path);
        update_download_directory_label(path);

    } catch (Error e) {
        if (!(e is Gtk.DialogError.DISMISSED)) {
            show_error(_("Failed to select directory: %s").printf(e.message));
        }
    }
}
```

**Rationale**:
- Gtk.FileDialog automatically uses portals in Flatpak
- Modern GTK 4.10+ API (already required by project)
- Async operation doesn't block UI
- Proper error handling for cancellation vs failure

**Security Benefits**:
- No direct filesystem access required
- Portal grants permission only for selected directory
- Follows principle of least privilege

### Decision 5: Settings Schema Structure

**New GSettings Keys**:
```xml
<key name="custom-download-directory" type="s">
  <default>""</default>
  <summary>Custom download directory</summary>
  <description>User-selected directory for downloads (empty for default xdg-download)</description>
</key>

<key name="download-notifications-enabled" type="b">
  <default>true</default>
  <summary>Enable download notifications</summary>
  <description>Whether to show toast notifications when downloads complete</description>
</key>
```

**Design Rationale**:
- Empty string for default maintains backward compatibility
- Separate notification toggle provides user control
- Follows existing settings pattern

### Decision 6: Download Directory Resolution Logic

**Priority Order**:
1. Custom directory (if set and accessible)
2. Default xdg-download
3. Fallback to ~/Downloads (if xdg-download unavailable)

**Implementation**:
```vala
private string get_download_directory() {
    // Check custom directory first
    var custom_dir = settings.get_string("custom-download-directory");
    if (custom_dir != "" && FileUtils.test(custom_dir, FileTest.IS_DIR)) {
        return custom_dir;
    }

    // Fall back to xdg-download
    var download_dir = Environment.get_user_special_dir(UserDirectory.DOWNLOAD);
    if (download_dir != null && FileUtils.test(download_dir, FileTest.IS_DIR)) {
        return download_dir;
    }

    // Ultimate fallback
    return Path.build_filename(Environment.get_home_dir(), "Downloads");
}
```

**Error Handling**:
- Graceful degradation if custom directory becomes unavailable
- User notification on fallback
- Automatic recovery without requiring user intervention

## File Organization

Following project conventions, new files will be created in:

```
src/managers/DownloadManager.vala    # New manager class
data/ui/preferences.blp               # Modified: Add download settings
data/io.github.tobagin.karere.gschema.xml.in  # Modified: Add settings keys
```

Modified files:
```
src/managers/WebViewManager.vala      # Add download policy handling
src/Window.vala                       # Add download toast display
src/Application.vala                  # Create DownloadManager instance
src/dialogs/PreferencesDialog.vala    # Add download preferences UI
```

## Signal Flow

### Download Completion Flow:

```
1. User clicks download link in WhatsApp Web
   ↓
2. WebView.decide_policy signal fires (RESPONSE type)
   ↓
3. WebViewManager detects download response
   ↓
4. WebViewManager emits download_detected signal
   ↓
5. DownloadManager receives signal
   ↓
6. DownloadManager resolves target directory
   ↓
7. WebKit downloads file to resolved directory
   ↓
8. Download completes (policy decision completes)
   ↓
9. DownloadManager emits download_completed signal
   ↓
10. Window receives signal and shows toast
    ↓
11. User clicks "Open" button (optional)
    ↓
12. DownloadManager.open_file() called
    ↓
13. AppInfo.launch_default_for_uri_async opens file
```

## Testing Strategy

### Unit Tests
- DownloadManager.get_download_directory() with various scenarios
- Directory accessibility checking
- Settings persistence and retrieval

### Integration Tests
- Custom directory selection workflow
- Download notification display
- File opening via portal
- Fallback behavior when custom directory unavailable

### Manual Testing Scenarios
1. Set custom download directory, download file, verify location
2. Delete custom directory, download file, verify fallback
3. Download file, click "Open" button, verify app launches
4. Disable download notifications, verify no toast appears
5. Test with various file types (images, documents, videos)

## Performance Considerations

### Minimal Overhead
- Download detection: Existing signal handler, negligible cost
- Directory resolution: Cached result, check only on change
- Toast display: Already used for other notifications
- File opening: Async operation, non-blocking

### Resource Usage
- DownloadManager: Singleton, minimal memory footprint
- No background monitoring or polling
- Event-driven architecture minimizes CPU usage

## Accessibility

### Screen Reader Support
- Toast messages announced automatically (Adwaita handles this)
- Action button keyboard accessible (Enter to activate)
- Preferences dialog fully keyboard navigable
- Clear labels for all interactive elements

### Keyboard Navigation
- File picker dialog: Full keyboard support (portal implementation)
- Toast action: Tab to button, Enter to activate
- Preferences: Standard tab order

## Future Enhancements (Out of Scope)

These are potential future improvements not included in this proposal:

1. **Download Progress**: Show progress in toast or separate UI
2. **Download History**: List of recent downloads with re-open capability
3. **Download Categories**: Automatic organization by file type
4. **Bandwidth Controls**: Limit download speeds or concurrent downloads
5. **Download Queue**: Manage multiple simultaneous downloads

These can be added later without breaking changes to the current design.

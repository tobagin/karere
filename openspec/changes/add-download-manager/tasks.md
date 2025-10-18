# Tasks: Add Download Manager

## Task List

### Phase 1: Foundation (Download Detection & Settings)

#### Task 1.1: Add GSettings Schema Keys
**Goal**: Add new settings keys for download directory and notifications
**Files**: `data/io.github.tobagin.karere.gschema.xml.in`
**Changes**:
- Add `custom-download-directory` string key with empty default
- Add `download-notifications-enabled` boolean key with true default
**Validation**: `meson test -C build validation` passes, schema compiles
**Dependencies**: None

#### Task 1.2: Create DownloadManager Class Skeleton
**Goal**: Create basic DownloadManager class structure
**Files**: `src/managers/DownloadManager.vala` (new)
**Changes**:
- Create class with Settings reference
- Define signals: download_detected, download_completed, download_failed, directory_fallback, error_opening_file
- Add constructor accepting Settings parameter
- Add stub methods: get_download_directory(), open_file()
**Validation**: File compiles without errors
**Dependencies**: Task 1.1 (needs schema keys)

#### Task 1.3: Implement Download Directory Resolution Logic
**Goal**: Implement logic to resolve download directory with fallback
**Files**: `src/managers/DownloadManager.vala`
**Changes**:
- Implement get_download_directory() method
- Add custom directory check with accessibility test
- Add xdg-download fallback logic
- Add ~/Downloads ultimate fallback
- Emit directory_fallback signal when needed
- Add settings change listener for custom-download-directory
**Validation**: Unit test or manual test with various directory states
**Dependencies**: Task 1.2

#### Task 1.4: Add DownloadManager to meson.build
**Goal**: Include DownloadManager in build system
**Files**: `meson.build`
**Changes**:
- Add `src/managers/DownloadManager.vala` to main sources list (around line 192)
- Ensure proper alphabetical ordering
**Validation**: `meson compile -C build` succeeds
**Dependencies**: Task 1.2

### Phase 2: WebView Integration (Download Detection)

#### Task 2.1: Add Download Detection to WebViewManager
**Goal**: Detect downloads via WebKit policy decisions
**Files**: `src/managers/WebViewManager.vala`
**Changes**:
- Add download_detected signal definition: `public signal void download_detected(string uri, string suggested_filename);`
- Enhance on_navigation_policy_decision() to handle RESPONSE type
- Add logic to detect attachments and downloadable MIME types
- Call decision.download() for downloads
- Emit download_detected signal with URI and filename
- Extract filename from Content-Disposition header or URI
**Validation**: Download a file from WhatsApp Web, verify signal fires
**Dependencies**: None (can work in parallel with Phase 1)

#### Task 2.2: Connect WebViewManager to DownloadManager
**Goal**: Wire up download detection signal to DownloadManager
**Files**: `src/Window.vala`, `src/managers/DownloadManager.vala`
**Changes**:
- In Window.vala, connect webview_manager.download_detected to download_manager handler
- In DownloadManager, add on_download_detected() method
- Store download metadata (filename, path) for completion handling
- Log download detection for debugging
**Validation**: Download file, verify DownloadManager receives notification
**Dependencies**: Task 2.1, Task 1.3

#### Task 2.3: Implement Download Completion Tracking
**Goal**: Track when downloads complete and emit completion signal
**Files**: `src/managers/DownloadManager.vala`, `src/managers/WebViewManager.vala`
**Changes**:
- In WebViewManager, connect to WebKit.Download signals (created, finished, failed)
- Track active downloads in DownloadManager
- Emit download_completed signal when download finishes
- Include filename and full path in signal
- Emit download_failed signal on errors
**Validation**: Download file, verify completion signal fires
**Dependencies**: Task 2.2

### Phase 3: Toast Notifications (UI Feedback)

#### Task 3.1: Implement Download Toast Display
**Goal**: Show toast notifications when downloads complete
**Files**: `src/Window.vala`
**Changes**:
- Add show_download_toast(string filename, string file_path) method
- Create AdwToast with download message
- Set timeout to 5 seconds
- Add to toast_overlay
- Connect to download_manager.download_completed signal
- Check download-notifications-enabled setting before showing
**Validation**: Download file, verify toast appears
**Dependencies**: Task 2.3

#### Task 3.2: Add Open File Action to Toast
**Goal**: Add action button to toast for opening files
**Files**: `src/Window.vala`
**Changes**:
- In show_download_toast(), add button_label = "Open"
- Create parameterized SimpleAction for opening files
- Set toast.action_name to the action
- Pass file path via action parameter or closure
- Add action to Window action group in setup_actions()
**Validation**: Download file, click "Open", verify file opens
**Dependencies**: Task 3.1

#### Task 3.3: Implement File Opening via Portal
**Goal**: Open downloaded files using default application
**Files**: `src/managers/DownloadManager.vala`
**Changes**:
- Implement open_file(string file_path) method
- Construct file:// URI from path
- Call AppInfo.launch_default_for_uri_async()
- Handle success and error cases
- Emit error_opening_file signal on failure
**Validation**: Click "Open" in toast, verify file opens in correct app
**Dependencies**: Task 3.2

#### Task 3.4: Add Error Toast for File Opening Failures
**Goal**: Show error messages when files can't be opened
**Files**: `src/Window.vala`
**Changes**:
- Connect to download_manager.error_opening_file signal
- Display error toast with message from signal
- Use existing show_error_toast() method
**Validation**: Trigger file opening error, verify error toast appears
**Dependencies**: Task 3.3

#### Task 3.5: Add Directory Fallback Toast
**Goal**: Notify user when custom directory falls back to default
**Files**: `src/Window.vala`
**Changes**:
- Connect to download_manager.directory_fallback signal
- Display warning toast: "Download directory unavailable, using default"
- Use 5 second timeout
**Validation**: Set invalid custom directory, download file, verify toast
**Dependencies**: Task 1.3

### Phase 4: Preferences UI (User Configuration)

#### Task 4.1: Add Downloads Preferences Page Structure
**Goal**: Create Downloads page in preferences dialog
**Files**: `data/ui/preferences.blp`
**Changes**:
- Add new AdwPreferencesPage with name="downloads"
- Set title to "Downloads"
- Add icon-name (e.g., "folder-download-symbolic")
- Create two AdwPreferencesGroup: "Download Directory" and "Notifications"
**Validation**: Open preferences, verify Downloads page appears
**Dependencies**: Task 1.1 (needs schema)

#### Task 4.2: Add Custom Directory Selection UI
**Goal**: Add UI elements for choosing custom download directory
**Files**: `data/ui/preferences.blp`
**Changes**:
- Add AdwActionRow in Download Directory group
- Set title to "Custom Directory"
- Add subtitle showing current directory or "Default (Downloads)"
- Add "Choose Folder" button as suffix
- Add "Reset to Default" button (visible when custom directory set)
- Bind row subtitle to display custom-download-directory or "Default"
**Validation**: View Downloads page, verify directory row appears correctly
**Dependencies**: Task 4.1

#### Task 4.3: Implement File Chooser Portal Integration
**Goal**: Open file chooser when "Choose Folder" is clicked
**Files**: `src/dialogs/PreferencesDialog.vala`
**Changes**:
- Add async select_download_directory() method
- Create Gtk.FileDialog for folder selection
- Set modal=true, title="Select Download Directory"
- Call select_folder() on preferences window
- On success, save path to custom-download-directory setting
- Update UI to show selected path
- Handle cancellation (Gtk.DialogError.DISMISSED) silently
- Handle other errors with error toast
**Validation**: Click "Choose Folder", select directory, verify saved
**Dependencies**: Task 4.2

#### Task 4.4: Implement Reset to Default Functionality
**Goal**: Allow resetting to default download directory
**Files**: `src/dialogs/PreferencesDialog.vala`, `data/ui/preferences.blp`
**Changes**:
- Connect "Reset to Default" button click signal
- Set custom-download-directory to empty string
- Update UI to show "Default (Downloads)"
- Hide/disable reset button when default is active
**Validation**: Set custom directory, click Reset, verify default restored
**Dependencies**: Task 4.3

#### Task 4.5: Add Download Notifications Toggle
**Goal**: Add switch to enable/disable download notifications
**Files**: `data/ui/preferences.blp`, `src/dialogs/PreferencesDialog.vala`
**Changes**:
- Add AdwActionRow in Notifications group
- Set title to "Show Download Notifications"
- Add subtitle explaining behavior
- Add GtkSwitch as activatable widget
- Bind switch to download-notifications-enabled setting
**Validation**: Toggle switch, verify setting changes, test with download
**Dependencies**: Task 4.1

#### Task 4.6: Update Preferences Navigation
**Goal**: Ensure Downloads page is in navigation sidebar
**Files**: `src/dialogs/PreferencesDialog.vala` (if manual navigation setup)
**Changes**:
- Verify Downloads page appears in sidebar navigation
- Ensure proper ordering (likely after Notifications or Spell Checking)
- Verify page switching works correctly
**Validation**: Navigate between preference pages
**Dependencies**: Task 4.1

### Phase 5: Application Integration

#### Task 5.1: Initialize DownloadManager in Application
**Goal**: Create DownloadManager singleton in Application
**Files**: `src/Application.vala`
**Changes**:
- Add private DownloadManager? download_manager field
- Create DownloadManager instance in startup() after settings initialization
- Pass settings reference to DownloadManager constructor
- Add getter method: public DownloadManager get_download_manager()
**Validation**: Run app, verify DownloadManager is created
**Dependencies**: Task 1.4

#### Task 5.2: Pass DownloadManager to Window
**Goal**: Provide DownloadManager reference to Window
**Files**: `src/Window.vala`, `src/Application.vala`
**Changes**:
- In Window constructor, get DownloadManager from Application
- Store reference in Window: private DownloadManager download_manager
- In Window.setup_notifications() or similar, connect download_manager signals
**Validation**: Download file, verify Window receives signals
**Dependencies**: Task 5.1, Phase 3 tasks

### Phase 6: Testing & Polish

#### Task 6.1: Manual Testing - Download Workflows
**Goal**: Test all download scenarios manually
**Test Cases**:
- Download image from WhatsApp Web → verify toast appears, file in correct directory
- Download document → click "Open" → verify opens in default app
- Download with custom directory set → verify file goes to custom location
- Download with notifications disabled → verify no toast appears
- Download with invalid custom directory → verify fallback toast and default directory used
**Validation**: All test cases pass
**Dependencies**: All previous tasks

#### Task 6.2: Manual Testing - Preferences UI
**Goal**: Test all preferences interactions
**Test Cases**:
- Open preferences → verify Downloads page exists
- Click "Choose Folder" → select directory → verify saved and displayed
- Click "Reset to Default" → verify reverts to default
- Toggle notifications switch → verify setting changes
- Change directory → download file → verify new directory used without restart
**Validation**: All test cases pass
**Dependencies**: Phase 4 tasks

#### Task 6.3: Accessibility Testing
**Goal**: Verify screen reader and keyboard accessibility
**Test Cases**:
- Navigate preferences with keyboard only
- Use screen reader (Orca) to navigate Downloads page
- Verify toast announcements with screen reader
- Tab to "Open" button in toast, activate with Enter
**Validation**: All accessibility features work
**Dependencies**: All UI tasks

#### Task 6.4: Error Handling Testing
**Goal**: Test edge cases and error conditions
**Test Cases**:
- Set custom directory, delete it, download file → verify fallback
- Download file, delete it, click "Open" → verify error toast
- Cancel file chooser dialog → verify no errors
- Download with no write permissions → verify appropriate error
**Validation**: Graceful error handling in all cases
**Dependencies**: All previous tasks

#### Task 6.5: Code Cleanup and Documentation
**Goal**: Polish code quality and add documentation
**Changes**:
- Add comprehensive doc comments to DownloadManager methods
- Add debug() statements for important operations
- Remove any temporary debug code
- Ensure consistent code style (4 spaces, snake_case)
- Add copyright headers if missing
**Validation**: Code review passes
**Dependencies**: All implementation tasks

#### Task 6.6: Build System Validation
**Goal**: Ensure clean builds in all configurations
**Test Cases**:
- `meson test -C build` passes all tests
- `meson compile -C build` completes without errors
- Development profile builds: `-Dprofile=development`
- Production profile builds: `-Dprofile=default`
- Flatpak build succeeds: `flatpak-builder build-dir packaging/*.yml`
**Validation**: All builds succeed
**Dependencies**: All tasks

## Parallelizable Work

These task groups can be worked on in parallel:
- **Group A**: Phase 1 (Settings & DownloadManager skeleton)
- **Group B**: Phase 2 (WebView integration) - can start in parallel with Phase 1
- **Group C**: Phase 4 (Preferences UI) - can start after Task 1.1

After Groups A, B, C complete:
- **Group D**: Phase 3 (Toast notifications) - needs Groups A and B
- **Group E**: Phase 5 (Application integration) - needs Groups A and D

Finally:
- **Group F**: Phase 6 (Testing) - needs all previous groups

## Estimated Time

- Phase 1: 3-4 hours
- Phase 2: 4-5 hours
- Phase 3: 3-4 hours
- Phase 4: 4-5 hours
- Phase 5: 1-2 hours
- Phase 6: 3-4 hours

**Total: 18-24 hours** (2-3 full working days)

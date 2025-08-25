# What's New Implementation Guide

This document explains how to implement a clean "What's New" feature that integrates release notes directly into the About dialog instead of using separate alert dialogs.

## Overview

Instead of showing annoying popup alerts for new versions, this approach:
- Opens the About dialog automatically on app launch (for new versions only)
- Navigates directly to the "What's New" section using programmatic tab+enter simulation
- Provides a unified, professional user experience

## Implementation Steps

### 1. Version Tracking with GSettings

Add a setting to track the last version shown to the user:

```xml
<!-- In your.gschema.xml.in -->
<key name="last-version-shown" type="s">
  <default>""</default>
  <summary>Last version for which release notes were shown</summary>
  <description>Tracks the last application version for which release notes were displayed to avoid showing them repeatedly</description>
</key>
```

### 2. Version Detection Logic

Create a method to check if this is a new version:

```vala
private bool should_show_release_notes() {
    var settings = new GLib.Settings(Config.APP_ID);
    string last_version = settings.get_string("last-version-shown");
    string current_version = Config.VERSION;

    // Show if this is the first run (empty last version) or version has changed
    if (last_version == "" || last_version != current_version) {
        settings.set_string("last-version-shown", current_version);
        return true;
    }
    return false;
}
```

### 3. Automatic Launch on App Startup

In your application's `activate()` method, check for new versions:

```vala
protected override void activate() {
    if (main_window == null) {
        main_window = new YourMainWindow(this);
    }
    
    main_window.present();
    
    // Check if this is a new version and show release notes automatically
    if (should_show_release_notes()) {
        // Small delay to ensure main window is fully presented
        Timeout.add(500, () => {
            // Launch about dialog with automatic navigation to release notes
            show_about_with_release_notes();
            return false;
        });
    }
}
```

### 4. Tab + Enter Navigation Implementation

Create methods to simulate keyboard navigation:

```vala
private void simulate_tab_navigation() {
    // Get the focused widget and try to move focus
    var focused_widget = main_window.get_focus();
    if (focused_widget != null) {
        // Use grab_focus to move to the next focusable widget
        var parent = focused_widget.get_parent();
        if (parent != null) {
            // Try to move focus to the next sibling
            parent.child_focus(Gtk.DirectionType.TAB_FORWARD);
        }
    }
}

private void simulate_enter_activation() {
    // Get the currently focused widget and try to activate it
    var focused_widget = main_window.get_focus();
    if (focused_widget != null) {
        // If it's a button, click it
        if (focused_widget is Button) {
            ((Button)focused_widget).activate();
        }
        // For other widgets, try to activate the default action
        else {
            focused_widget.activate_default();
        }
    }
}
```

### 5. About Dialog with Navigation

Create a method that opens the about dialog and navigates to release notes:

```vala
private void show_about_with_release_notes() {
    // Open the about dialog first
    on_about_action();
    
    // Wait for the dialog to appear, then navigate to release notes
    Timeout.add(300, () => {
        simulate_tab_navigation();
        
        // Simulate Enter key press after another delay to open release notes
        Timeout.add(200, () => {
            simulate_enter_activation();
            return false;
        });
        return false;
    });
}
```

### 6. Standard About Dialog (No Auto-Navigation)

Keep your regular about dialog method clean for manual access:

```vala
private void on_about_action() {
    // Standard about dialog creation
    var about = new Adw.AboutDialog() {
        application_name = "Your App",
        application_icon = Config.APP_ID,
        developer_name = "Your Name",
        version = Config.VERSION,
        // ... other properties
    };

    // Load release notes from metainfo (standard LibAdwaita approach)
    try {
        var metainfo_path = Path.build_filename(Config.DATADIR, "metainfo", "%s.metainfo.xml".printf(Config.APP_ID));
        var file = File.new_for_path(metainfo_path);
        
        if (file.query_exists()) {
            uint8[] contents;
            file.load_contents(null, out contents, null);
            string xml_content = (string) contents;
            
            // Parse XML and extract release notes for current version
            // Set release notes with: about.set_release_notes(release_notes);
        }
    } catch (Error e) {
        warning("Could not load release notes: %s", e.message);
    }

    if (main_window != null) {
        about.present(main_window);
    }
}
```

### 7. Metainfo Integration

Ensure your `metainfo.xml` contains release information:

```xml
<releases>
  <release version="1.2.0" date="2024-01-15">
    <description>
      <p>New features in this release:</p>
      <ul>
        <li>Added dark mode support</li>
        <li>Improved performance by 50%</li>
        <li>Fixed critical bug with data saving</li>
      </ul>
    </description>
  </release>
</releases>
```

## Key Benefits

1. **Professional UX**: No annoying separate popups
2. **Unified Experience**: Everything in one about dialog
3. **Automatic but Optional**: Shows automatically for new versions, but users can still access manually
4. **Standard Compliant**: Uses LibAdwaita's built-in release notes feature
5. **Clean Code**: Separates automatic vs manual about dialog logic

## Testing

To test the functionality:

```bash
# Set to simulate upgrade from older version
flatpak run --command=gsettings your.app.id set your.app.id last-version-shown "1.1.0"

# Run app - should auto-show release notes
flatpak run your.app.id

# Reset to current version to stop auto-showing
flatpak run --command=gsettings your.app.id set your.app.id last-version-shown "1.2.0"
```

## Timing Considerations

- **App Launch Delay**: 500ms to ensure main window is presented
- **Dialog Appearance**: 300ms for about dialog to fully render
- **Navigation Delay**: 200ms between tab and enter to ensure proper focus

These timings work well but can be adjusted based on your app's performance characteristics.

## Troubleshooting

1. **Navigation not working**: Check if the about dialog has the expected button layout
2. **Version not detected**: Verify gsettings schema is properly installed
3. **Multiple triggers**: Ensure the setting is updated immediately after detection

This approach provides a much cleaner user experience compared to separate alert dialogs while maintaining the same functionality.
# Preferences and Configuration Guide

> Complete guide to customizing and configuring Karere

## Accessing Preferences

### Opening the Preferences Dialog

Access Karere's preferences through any of these methods:

1. **Keyboard Shortcut**: Press `Ctrl+,`
2. **Application Menu**: Click the application menu button in the header bar → Preferences
3. **Primary Menu**: Access through the three-dot menu (if available)

The preferences dialog uses LibAdwaita's modern design with organized pages and groups for easy navigation.

## Appearance Settings

### Theme Configuration

Karere integrates seamlessly with your desktop environment's theming system.

#### Theme Options

**System Theme (Default)**
- Automatically follows your system's Light/Dark preference
- Changes dynamically when you switch system themes
- Recommended for most users

**Light Theme**
- Forces light appearance regardless of system settings
- Better for bright environments or daylight usage
- Reduces eye strain in well-lit conditions

**Dark Theme**
- Forces dark appearance regardless of system settings
- Better for low-light environments
- May reduce battery usage on OLED displays
- Popular for late-night usage

#### Applying Theme Changes

1. Open **Preferences → Appearance**
2. Select your preferred theme option
3. Changes apply immediately without restart

#### Custom Theme Support

For advanced users, Karere supports custom GTK CSS theming:

```css
/* ~/.var/app/io.github.tobagin.karere/config/gtk-4.0/gtk.css */

/* Custom window styling */
.karere-window {
    background-color: #f5f5f5;
    border-radius: 8px;
}

/* Custom header bar */
.karere-headerbar {
    background: linear-gradient(to bottom, #e0e0e0, #d0d0d0);
}

/* Custom button styling */
.karere-button {
    border-radius: 6px;
    padding: 8px 16px;
}
```

### Window and Display Settings

#### Window Behavior
- **Remember Window Size**: Karere automatically saves and restores window dimensions
- **Remember Window Position**: Window position is preserved between sessions
- **Maximize State**: Remembers if window was maximized

#### Display Scaling
Karere respects your system's display scaling settings:
- **Automatic Scaling**: Follows system DPI settings
- **High DPI Support**: Proper scaling on high-resolution displays
- **Font Scaling**: Respects system font size preferences

## Notification Settings

### Notification Configuration

Karere provides comprehensive notification management through **Preferences → Notifications**.

#### Master Notification Control

**Enable Notifications**
- Toggle to enable/disable all notifications
- When disabled, no desktop notifications will appear
- WhatsApp Web's internal notifications are also suppressed

#### Notification Types

**Message Notifications**
- **Individual Chats**: Notifications for one-on-one conversations
- **Group Chats**: Notifications for group messages
- **Broadcast Lists**: Notifications for broadcast messages

**Call Notifications**
- **Incoming Calls**: Voice and video call notifications
- **Missed Calls**: Notifications for missed calls

**Status Notifications**
- **Status Updates**: When contacts post new status updates
- **System Messages**: Connection status and system notifications

#### Notification Content

**Message Preview Options**
- **Full Preview**: Show sender name and message content
- **Sender Only**: Show only sender name, hide message content
- **Private Mode**: Show only "New message" without details
- **No Preview**: Minimal notification with no content

**Preview Examples**:
- *Full*: "John Doe: Hey, are we still meeting at 3 PM?"
- *Sender Only*: "John Doe sent a message"
- *Private*: "New message received"
- *No Preview*: "Karere" (application name only)

#### Notification Behavior

**Notification Sounds**
- **Enable Sounds**: Toggle notification sound effects
- **System Sounds**: Use system notification sounds
- **Custom Sounds**: Configure custom notification tones (advanced)

**Do Not Disturb Integration**
- **Respect System DND**: Honor system Do Not Disturb settings
- **Custom DND Hours**: Set specific quiet hours for Karere
- **Priority Contacts**: Allow notifications from specific contacts during DND

**Notification Persistence**
- **Auto-dismiss**: Automatically dismiss notifications after timeout
- **Timeout Duration**: Configure how long notifications remain visible
- **Notification History**: Integration with system notification center

#### Advanced Notification Settings

**Priority and Grouping**
- **High Priority Messages**: Mark important messages for priority notifications
- **Group Similar Notifications**: Combine multiple messages from same sender
- **Notification Stacking**: Stack multiple notifications to avoid clutter

**Action Buttons** (when supported by desktop environment)
- **Quick Reply**: Reply directly from notification
- **Mark as Read**: Mark messages as read without opening app
- **Mute Chat**: Temporarily mute specific conversations

### Platform-Specific Notification Settings

#### GNOME
- Notifications integrate with GNOME Shell notification system
- Automatic Do Not Disturb during presentations/fullscreen
- Integration with GNOME Control Center notification settings

#### KDE Plasma
- Integration with KDE notification system
- Custom notification themes through Plasma settings
- Support for notification actions

#### Other Desktop Environments
- Basic notification support through libnotify
- Limited advanced features depending on DE capabilities

## Privacy Settings

### Privacy Configuration

Access privacy controls through **Preferences → Privacy**.

#### Logging Settings

**Debug Logging**
- **Enable Debug Logging**: Toggle detailed debug information
- **Log Level**: Configure verbosity (Error, Warning, Info, Debug)
- **Log Categories**: Enable logging for specific components
  - Application lifecycle events
  - WebKit rendering information
  - Network activity
  - Notification system
  - Preferences changes

**Log Management**
- **Log File Location**: View where logs are stored
- **Log Retention Period**: Configure how long logs are kept (7-90 days)
- **Maximum Log Size**: Set size limits for log files
- **Automatic Cleanup**: Enable/disable automatic old log deletion

**Log Access**
- **View Current Logs**: Button to open log viewer
- **Export Logs**: Save logs for bug reporting or analysis
- **Clear All Logs**: Button to delete all stored logs

#### Crash Reporting

**Crash Detection**
- **Enable Crash Reporting**: Opt-in to crash detection and reporting
- **Automatic Detection**: System monitors for application crashes
- **Manual Reporting**: Option to manually submit crash reports

**Crash Data Collection**
When enabled, crash reports may include:
- Stack trace and error information
- System configuration (OS, desktop environment)
- Application state at time of crash
- Hardware information (CPU, memory)

**Never Included in Crash Reports:**
- Personal messages or chat content
- Login credentials or session tokens
- Personal files or documents
- Network traffic content
- Contact information

**Crash Report Handling**
- **Review Before Sending**: Always review crash data before submission
- **Anonymous Submission**: No identifying information included
- **Secure Transmission**: Encrypted upload to crash reporting service
- **Data Retention**: Crash reports stored temporarily for analysis

#### Data Management

**Application Data**
- **Local Storage Location**: Display where application data is stored
- **Data Types**: Information about what data is stored locally
- **Clear Application Data**: Button to reset all application data

**Cache Management**
- **Cache Size**: Display current cache usage
- **Cache Types**: Web cache, image cache, temporary files
- **Clear Cache**: Button to clear all cached data
- **Automatic Cache Cleanup**: Enable periodic cache cleaning

**Session Data**
- **Login Sessions**: Manage WhatsApp Web login sessions
- **Cookie Management**: Control cookie storage and retention
- **Clear Sessions**: Button to log out and clear session data

## Advanced Settings

### Network Configuration

#### Connection Settings
- **Proxy Configuration**: Configure HTTP/HTTPS proxy settings
- **User Agent**: Custom user agent string (advanced users)
- **Connection Timeout**: Network timeout settings
- **Retry Behavior**: Configure connection retry attempts

#### WebKit Settings
- **JavaScript**: Enable/disable JavaScript execution
- **Images**: Control image loading behavior
- **Cache Settings**: WebKit-specific cache configuration
- **Security Settings**: Additional web security options

### Performance Settings

#### Resource Management
- **Memory Limits**: Set maximum memory usage (advanced)
- **CPU Priority**: Adjust process priority
- **Hardware Acceleration**: Enable/disable graphics acceleration
- **Background Processing**: Control background activity

#### Optimization Options
- **Reduce Animations**: Minimize visual effects for performance
- **Low Resource Mode**: Optimizations for constrained systems
- **Battery Optimization**: Settings for laptop users

### Developer Settings

#### Debug Features
- **Developer Tools**: Enable WebKit inspector
- **Console Logging**: Additional console output
- **Performance Profiling**: Enable performance monitoring
- **Network Inspection**: Monitor network requests

#### Experimental Features
- **Beta Features**: Access to experimental functionality
- **Advanced Options**: Power user settings
- **Custom Configurations**: Manual configuration overrides

## Configuration Files

### File Locations

Karere stores configuration in standard XDG directories:

```
~/.var/app/io.github.tobagin.karere/
├── config/
│   ├── karere.conf          # Main configuration
│   ├── notifications.conf   # Notification settings
│   ├── privacy.conf         # Privacy preferences
│   └── gtk-4.0/
│       └── gtk.css          # Custom CSS themes
├── data/
│   ├── webkit/              # WebKit data
│   ├── logs/                # Application logs
│   └── sessions/            # Login sessions
└── cache/                   # Temporary cache files
```

### Manual Configuration

#### Main Configuration File

The main configuration is stored in INI format:

```ini
# ~/.var/app/io.github.tobagin.karere/config/karere.conf

[appearance]
theme=system
follow-system-theme=true
enable-animations=true

[window]
remember-size=true
remember-position=true
width=1200
height=800
maximized=false

[notifications]
enabled=true
show-message-preview=true
enable-sounds=true
respect-dnd=true

[privacy]
debug-logging=false
crash-reporting=false
log-retention-days=30
```

#### Notification Configuration

```ini
# ~/.var/app/io.github.tobagin.karere/config/notifications.conf

[general]
enabled=true
sounds=true
preview-mode=full

[message-types]
individual-chats=true
group-chats=true
broadcast-lists=true

[call-types]
incoming-calls=true
missed-calls=true

[dnd]
respect-system-dnd=true
custom-quiet-hours=false
quiet-start=22:00
quiet-end=08:00
```

#### Privacy Configuration

```ini
# ~/.var/app/io.github.tobagin.karere/config/privacy.conf

[logging]
debug-enabled=false
log-level=warning
retention-days=30
max-log-size=50MB
categories=application,webkit,notifications

[crash-reporting]
enabled=false
anonymous-only=true
include-system-info=true

[data-management]
auto-cleanup-cache=true
cache-cleanup-interval=7days
max-cache-size=500MB
```

### Backup and Restore

#### Backing Up Configuration

```bash
# Create configuration backup
tar -czf karere-config-backup.tar.gz \
  ~/.var/app/io.github.tobagin.karere/config/

# Backup specific settings only
cp ~/.var/app/io.github.tobagin.karere/config/karere.conf \
   karere-settings-backup.conf
```

#### Restoring Configuration

```bash
# Restore full configuration
tar -xzf karere-config-backup.tar.gz -C ~/

# Restore specific settings
cp karere-settings-backup.conf \
   ~/.var/app/io.github.tobagin.karere/config/karere.conf
```

#### Reset to Defaults

```bash
# Reset all configuration to defaults
rm -rf ~/.var/app/io.github.tobagin.karere/config/

# Reset specific configuration sections
# (Will be recreated with defaults on next launch)
rm ~/.var/app/io.github.tobagin.karere/config/karere.conf
```

## Troubleshooting Configuration Issues

### Common Configuration Problems

#### Settings Not Persisting
If settings aren't being saved:
1. Check file permissions in config directory
2. Ensure sufficient disk space
3. Verify config directory isn't read-only

```bash
# Fix permissions
chmod -R 755 ~/.var/app/io.github.tobagin.karere/config/
```

#### Invalid Configuration
If Karere won't start due to invalid config:
1. Check configuration file syntax
2. Reset to defaults if necessary
3. Check application logs for specific errors

```bash
# Validate configuration
# (Run Karere with debug output)
KARERE_DEBUG=1 flatpak run io.github.tobagin.karere
```

#### Theme Not Applying
If theme changes don't take effect:
1. Restart Karere completely
2. Check GTK theme compatibility
3. Verify system theme is properly set

```bash
# Check system theme setting
gsettings get org.gnome.desktop.interface color-scheme
```

### Configuration Migration

When upgrading Karere versions, configuration is automatically migrated. If you encounter issues:

1. **Backup existing config** before upgrading
2. **Check migration logs** in application logs
3. **Reset problematic sections** if needed
4. **Report migration issues** to developers

## Expert Configuration

### Advanced Customization

#### Custom Keyboard Shortcuts

```ini
# Add to karere.conf
[keyboard]
preferences=<Ctrl>comma
quit=<Ctrl>q
reload=<Ctrl>r
fullscreen=F11
developer-tools=<Ctrl><Shift>i
```

#### Network Optimization

```ini
# Network tuning
[network]
user-agent=custom-user-agent-string
connection-timeout=30
max-retries=3
enable-http2=true
```

#### WebKit Customization

```ini
# WebKit engine settings
[webkit]
enable-javascript=true
enable-plugins=false
enable-webgl=false
enable-accelerated-compositing=true
cache-model=web-browser
```

---

*For troubleshooting configuration issues, see the [Troubleshooting Guide](../troubleshooting/common-issues.md) or [FAQ](../FAQ.md).*
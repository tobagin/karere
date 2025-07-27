# Keyboard Shortcuts Guide

> Complete reference for Karere keyboard shortcuts and navigation

## Quick Reference

### Essential Shortcuts

| Shortcut | Action | Description |
|----------|--------|-------------|
| `Ctrl+,` | Preferences | Open application preferences |
| `Ctrl+Q` | Quit | Close Karere completely |
| `F11` | Fullscreen | Toggle fullscreen mode |
| `Ctrl+R` | Reload | Reload WhatsApp Web interface |
| `Alt+F4` | Close Window | Close application window |

### Navigation Shortcuts

| Shortcut | Action | Description |
|----------|--------|-------------|
| `Tab` | Next Element | Navigate to next interactive element |
| `Shift+Tab` | Previous Element | Navigate to previous interactive element |
| `Enter` | Activate | Activate focused element |
| `Space` | Toggle | Toggle focused checkboxes/switches |
| `Escape` | Cancel/Close | Close dialogs or cancel operations |

## Application Shortcuts

### Window Management

#### Basic Window Operations
- **`Ctrl+Q`**: Quit application
  - Closes Karere completely
  - Saves current session state
  - Can be configured to minimize to tray instead

- **`Alt+F4`**: Close window
  - Standard window close operation
  - Same effect as clicking the X button
  - Prompts for confirmation if configured

- **`F11`**: Toggle fullscreen
  - Expands Karere to fill entire screen
  - Hides window decorations and header bar
  - Press `F11` again to exit fullscreen

#### Window State
- **`Super+Up`**: Maximize window (system shortcut)
- **`Super+Down`**: Restore/minimize window (system shortcut)
- **`Super+Left/Right`**: Tile window to sides (system shortcut)

### Application Features

#### Interface Control
- **`Ctrl+,`**: Open preferences
  - Opens the main preferences dialog
  - Navigate with Tab/Shift+Tab between options
  - Changes apply immediately

- **`Ctrl+R`**: Reload WhatsApp Web
  - Refreshes the WhatsApp Web interface
  - Useful when experiencing connection issues
  - Does not log you out of WhatsApp

- **`Ctrl+Shift+R`**: Hard reload (if supported)
  - Forces complete reload of web content
  - Clears temporary cache
  - May require re-login

#### Developer Features
- **`Ctrl+Shift+I`**: Open developer tools
  - Opens WebKit inspector (if enabled in preferences)
  - Useful for debugging web interface issues
  - Only available when developer mode is enabled

- **`F12`**: Alternative developer tools shortcut
  - Same as `Ctrl+Shift+I`
  - More familiar for web developers

## WhatsApp Web Shortcuts

Since Karere displays the actual WhatsApp Web interface, all standard WhatsApp Web keyboard shortcuts work within the application.

### Chat Navigation

#### Chat List
- **`Ctrl+K`**: Search or start new chat
- **`Up/Down Arrow`**: Navigate chat list
- **`Enter`**: Open selected chat
- **`Ctrl+N`**: Start new chat
- **`Ctrl+Shift+N`**: Start new group

#### Message Navigation
- **`Page Up/Page Down`**: Scroll through messages
- **`Home/End`**: Go to beginning/end of chat
- **`Ctrl+F`**: Search in current chat
- **`Escape`**: Clear search or exit current view

### Message Composition

#### Text Formatting
- **`Ctrl+B`**: Bold text (*bold*)
- **`Ctrl+I`**: Italic text (_italic_)
- **`Ctrl+Shift+X`**: Strikethrough text (~strikethrough~)
- **`Ctrl+Shift+M`**: Monospace text (```monospace```)

#### Message Actions
- **`Enter`**: Send message
- **`Shift+Enter`**: New line in message
- **`Ctrl+Enter`**: Send message (alternative)
- **`Up Arrow`**: Edit last sent message (when text box is empty)

#### Emoji and Media
- **`Ctrl+E`**: Open emoji picker
- **`Ctrl+Shift+C`**: Open camera
- **`Ctrl+Shift+D`**: Open document picker
- **`Ctrl+Shift+A`**: Attach media

### Call Shortcuts
- **`Ctrl+Shift+V`**: Start voice call
- **`Ctrl+Shift+C`**: Start video call
- **`Space`**: Mute/unmute during call
- **`Ctrl+E`**: Turn camera on/off during video call

## Accessibility Shortcuts

### Screen Reader Support

Karere fully supports screen readers and assistive technologies:

#### Navigation with Screen Readers
- **`Tab`**: Move to next interactive element
- **`Shift+Tab`**: Move to previous interactive element
- **`Arrow Keys`**: Navigate within elements (lists, menus)
- **`Enter/Space`**: Activate focused element

#### Screen Reader Specific
- **`Ctrl+Alt+H`**: Announce heading level (NVDA/JAWS)
- **`Ctrl+Alt+L`**: Announce current line (NVDA/JAWS)
- **`Insert+T`**: Read title bar (NVDA)
- **`Insert+B`**: Read status bar (NVDA)

### High Contrast and Visual Accessibility
- **System theme shortcuts**: Use system shortcuts to toggle high contrast
- **Zoom**: `Ctrl++` and `Ctrl+-` for zooming (if supported by WebKit)
- **System magnifier**: Use system-level magnification tools

## Custom Shortcuts

### Configuring Custom Shortcuts

While Karere uses standard shortcuts, you can configure custom system-level shortcuts:

#### Using GNOME Settings
1. Open **Settings → Keyboard → Keyboard Shortcuts**
2. Click **Custom Shortcuts** → **+**
3. Set name: "Open Karere"
4. Set command: `flatpak run io.github.tobagin.karere`
5. Set shortcut: Choose your preferred key combination

#### Using gsettings (command line)
```bash
# Create custom shortcut to launch Karere
gsettings set org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/custom0/ name 'Karere'
gsettings set org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/custom0/ command 'flatpak run io.github.tobagin.karere'
gsettings set org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/custom0/ binding '<Super>w'
```

### Application-Specific Shortcuts

You can also create shortcuts for specific Karere actions:

```bash
# Quick reload Karere (kills and restarts)
# Create script: ~/bin/karere-reload
#!/bin/bash
pkill -f karere
sleep 1
flatpak run io.github.tobagin.karere &

# Then bind this script to a keyboard shortcut
```

## Platform-Specific Shortcuts

### GNOME Desktop

#### GNOME Shell Integration
- **`Super`**: Open Activities overview (shows Karere if running)
- **`Alt+Tab`**: Switch between applications (includes Karere)
- **`Alt+F2`**: Run command (type "karere" to launch)
- **`Super+A`**: Show applications grid

#### GNOME Specific Features
- **`Super+H`**: Hide window (minimize)
- **`Super+M`**: Open notification center (shows Karere notifications)
- **`Ctrl+Alt+D`**: Show desktop (hides all windows)

### KDE Plasma

#### KDE Integration
- **`Meta`**: Open Application Launcher
- **`Alt+F2`**: KRunner (type "karere")
- **`Ctrl+Alt+T`**: Open terminal (to launch Karere from command line)

#### KWin Window Manager
- **`Meta+Left/Right`**: Tile windows
- **`Meta+Up`**: Maximize window
- **`Meta+Down`**: Minimize window

### Other Desktop Environments

#### XFCE
- **`Alt+F1`**: Applications menu
- **`Alt+F2`**: Application finder
- **`Ctrl+Alt+D`**: Show desktop

#### i3/Sway (Tiling Window Managers)
- **`$mod+d`**: Launch dmenu/rofi (type "karere")
- **`$mod+Enter`**: Open terminal (to launch Karere)
- **`$mod+Shift+Q`**: Close focused window

## Troubleshooting Shortcuts

### When Shortcuts Don't Work

#### Check Conflicting Shortcuts
1. **System shortcuts take precedence** over application shortcuts
2. **Check desktop environment settings** for conflicting bindings
3. **Test in different applications** to isolate the issue

#### Debug Shortcut Issues
```bash
# Check if Karere is receiving keyboard events
# (Run from terminal to see debug output)
KARERE_DEBUG=1 flatpak run io.github.tobagin.karere

# Monitor keyboard events system-wide
evtest  # Select keyboard device and test keys

# Check GTK key theme
gsettings get org.gnome.desktop.interface gtk-key-theme
```

#### Reset Shortcuts
```bash
# Reset GNOME keyboard shortcuts
gsettings reset-recursively org.gnome.desktop.wm.keybindings
gsettings reset-recursively org.gnome.settings-daemon.plugins.media-keys

# Reset GTK accelerators
rm ~/.config/gtk-3.0/accels
rm ~/.config/gtk-4.0/accels
```

### Accessibility Issues

#### Screen Reader Problems
1. **Ensure screen reader is running** before launching Karere
2. **Check AT-SPI accessibility** is enabled:
   ```bash
   gsettings get org.gnome.desktop.interface toolkit-accessibility
   # Should return 'true'
   ```
3. **Test with Orca** (GNOME's built-in screen reader):
   ```bash
   orca --replace &
   ```

#### Keyboard Navigation Issues
1. **Enable focus indicators** in system settings
2. **Check tab order** in the interface
3. **Test with different themes** (some may hide focus indicators)

## Advanced Keyboard Usage

### Power User Tips

#### Combining Shortcuts
- **`Ctrl+,` → Tab → Enter**: Quick access to first preference option
- **`Ctrl+R` → `Ctrl+,`**: Reload and immediately open preferences
- **`F11` → `Alt+Tab`**: Fullscreen then switch apps (for multi-tasking)

#### Workflow Optimization
1. **Use system launcher** (`Super` key) to quickly launch Karere
2. **Pin to taskbar/dock** for one-click access
3. **Create desktop shortcut** for quick access
4. **Use workspace shortcuts** to organize applications

#### Multiple Instance Management
```bash
# Launch multiple instances (if needed for different accounts)
flatpak run io.github.tobagin.karere &  # Instance 1
flatpak run io.github.tobagin.karere.Devel &  # Instance 2 (development)

# Use Alt+Tab to switch between instances
```

### Keyboard-Only Usage

For users who prefer or require keyboard-only operation:

#### Complete Keyboard Workflow
1. **Launch**: `Super` → Type "karere" → `Enter`
2. **Navigate**: Use `Tab`/`Shift+Tab` for all navigation
3. **Settings**: `Ctrl+,` → Navigate with arrows/Tab
4. **WhatsApp**: Use WhatsApp Web shortcuts within the interface
5. **Close**: `Ctrl+Q` or `Alt+F4`

#### Tips for Keyboard-Only Users
- **Learn WhatsApp Web shortcuts** - they all work in Karere
- **Use system shortcuts** for window management
- **Enable focus indicators** for better visibility
- **Consider accessibility themes** for better contrast

## Reference Card

### Printable Quick Reference

```
KARERE KEYBOARD SHORTCUTS QUICK REFERENCE

Application Control:
Ctrl+,     Preferences        F11        Fullscreen
Ctrl+Q     Quit              Ctrl+R     Reload
Alt+F4     Close Window      Escape     Cancel/Close

Navigation:
Tab        Next Element      Enter      Activate
Shift+Tab  Previous Element  Space      Toggle

WhatsApp Web (within interface):
Ctrl+K     Search/New Chat   Ctrl+B     Bold Text
Ctrl+N     New Chat          Ctrl+I     Italic Text
Ctrl+F     Search in Chat    Ctrl+E     Emoji Picker
Enter      Send Message      Shift+Enter New Line

Developer:
Ctrl+Shift+I  Developer Tools  F12       Developer Tools

System Integration:
Super      Activities/Launcher  Alt+Tab   Switch Apps
Super+H    Hide Window         Super+M   Notifications
```

*Print this reference card and keep it handy while learning Karere shortcuts!*

---

*For additional keyboard accessibility features, see the [Features Guide](features.md#accessibility-features) or [FAQ](../FAQ.md).*
# Common Issues and Troubleshooting

> Solutions for frequently encountered problems with Karere

## Quick Diagnosis

If you're experiencing issues with Karere, try these quick diagnostic steps first:

1. **Check System Requirements**: Ensure your system meets [minimum requirements](../user-guide/getting-started.md#system-requirements)
2. **Restart Application**: Close and reopen Karere completely
3. **Check Internet Connection**: Verify you have stable internet connectivity
4. **Update Application**: Ensure you're running the latest version
5. **Review Logs**: Check application logs for error messages

## Installation Issues

### Flatpak Installation Problems

#### Issue: "No such ref" error when installing
```bash
error: No such ref 'flathub:app/io.github.tobagin.karere/x86_64/stable' in remote 'flathub'
```

**Solution:**
1. Ensure Flathub repository is added:
   ```bash
   flatpak remote-add --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo
   ```
2. Update Flatpak repositories:
   ```bash
   flatpak update --appstream
   ```
3. Try installation again

#### Issue: Permission denied during installation
```bash
error: While trying to write /var/lib/flatpak/...: Permission denied
```

**Solution:**
1. Install as user instead of system-wide:
   ```bash
   flatpak install --user flathub io.github.tobagin.karere
   ```
2. Or ensure you have proper sudo permissions:
   ```bash
   sudo flatpak install flathub io.github.tobagin.karere
   ```

#### Issue: Flatpak runtime missing
```bash
error: Required runtime org.gnome.Sdk/x86_64/48 is not installed
```

**Solution:**
```bash
flatpak install flathub org.gnome.Sdk//48
flatpak install flathub org.gnome.Platform//48
```

### Source Build Issues

#### Issue: Missing dependencies during build
```bash
meson.build:XX:X: ERROR: Dependency "gtk4" not found
```

**Solution by Distribution:**

**Ubuntu/Debian:**
```bash
sudo apt update
sudo apt install libgtk-4-dev libadwaita-1-dev libwebkitgtk-6.0-dev
```

**Fedora:**
```bash
sudo dnf install gtk4-devel libadwaita-devel webkitgtk6.0-devel
```

**Arch Linux:**
```bash
sudo pacman -S gtk4 libadwaita webkitgtk-6.0
```

#### Issue: Vala compiler not found
```bash
meson.build:XX:X: ERROR: Program 'valac' not found
```

**Solution:**
```bash
# Ubuntu/Debian
sudo apt install valac

# Fedora
sudo dnf install vala

# Arch Linux
sudo pacman -S vala
```

#### Issue: Blueprint compiler missing
```bash
ERROR: Program 'blueprint-compiler' not found
```

**Solution:**
```bash
# Ubuntu/Debian (22.04+)
sudo apt install blueprint-compiler

# Fedora
sudo dnf install blueprint-compiler

# Arch Linux
sudo pacman -S blueprint-compiler

# Manual installation if not available
pip install --user blueprint-compiler
```

## Launch and Startup Issues

### Application Won't Start

#### Issue: Karere launches but shows blank window
This usually indicates a WebKitGTK or network connectivity issue.

**Solution:**
1. Check internet connection
2. Verify WebKitGTK installation:
   ```bash
   # Check if WebKitGTK is properly installed
   flatpak info org.gnome.Platform//48 | grep webkit
   ```
3. Clear application cache:
   ```bash
   # For Flatpak version
   rm -rf ~/.var/app/io.github.tobagin.karere/cache/*
   ```
4. Check system logs:
   ```bash
   journalctl --user -f | grep karere
   ```

#### Issue: Application crashes immediately on startup
```bash
Segmentation fault (core dumped)
```

**Solution:**
1. Enable debug logging (if possible):
   ```bash
   KARERE_DEBUG=1 flatpak run io.github.tobagin.karere
   ```
2. Check for conflicting applications
3. Verify all dependencies are installed
4. Report crash with stack trace to developers

#### Issue: "Failed to load WebKit" error
```bash
ERROR: Failed to initialize WebKit
```

**Solution:**
1. Update WebKitGTK:
   ```bash
   # Flatpak
   flatpak update org.gnome.Platform
   
   # Native
   sudo apt update && sudo apt upgrade webkitgtk*  # Ubuntu/Debian
   sudo dnf update webkitgtk*                      # Fedora
   ```
2. Check for conflicting WebKit versions
3. Restart system to ensure proper library loading

### Slow Startup

#### Issue: Application takes a long time to start
This can be caused by several factors.

**Solutions:**
1. **Clear cache data:**
   ```bash
   rm -rf ~/.var/app/io.github.tobagin.karere/cache/*
   ```

2. **Disable unnecessary extensions/plugins**

3. **Check available memory:**
   ```bash
   free -h
   ```

4. **Monitor startup performance:**
   ```bash
   time flatpak run io.github.tobagin.karere
   ```

## WhatsApp Web Connection Issues

### Cannot Connect to WhatsApp Web

#### Issue: "Could not connect to WhatsApp Web" error
This indicates network or WhatsApp service issues.

**Solutions:**
1. **Check WhatsApp Web status:**
   - Open https://web.whatsapp.com in a regular browser
   - Verify the service is operational

2. **Network connectivity:**
   ```bash
   # Test connectivity to WhatsApp servers
   ping web.whatsapp.com
   curl -I https://web.whatsapp.com
   ```

3. **Clear application data:**
   ```bash
   # This will log you out of WhatsApp
   rm -rf ~/.var/app/io.github.tobagin.karere/data/*
   ```

4. **Check firewall settings:**
   - Ensure port 443 (HTTPS) is open
   - Check corporate firewall/proxy settings

#### Issue: QR Code not appearing or not scannable

**Solutions:**
1. **Refresh the page:**
   - Press `Ctrl+R` to reload WhatsApp Web

2. **Check display scaling:**
   - Ensure display scaling doesn't affect QR code readability
   - Try different window sizes

3. **Clear cookies:**
   ```bash
   rm -rf ~/.var/app/io.github.tobagin.karere/data/webkit/*
   ```

4. **Use alternative login method:**
   - Try "Link with phone number instead" option

### Frequent Disconnections

#### Issue: WhatsApp Web keeps disconnecting

**Solutions:**
1. **Check phone connectivity:**
   - Ensure your phone has stable internet connection
   - Keep WhatsApp open on your phone

2. **Network stability:**
   ```bash
   # Monitor network stability
   ping -c 100 8.8.8.8
   ```

3. **Power management:**
   - Disable system sleep/hibernate while using Karere
   - Check laptop power management settings

4. **WhatsApp Web limitations:**
   - WhatsApp Web sessions expire after 14 days of phone inactivity
   - Re-link your device if necessary

## Notification Issues

### Notifications Not Appearing

#### Issue: No desktop notifications for new messages

**Solutions:**
1. **Check notification permissions:**
   ```bash
   # GNOME Settings
   gnome-control-center notifications
   ```

2. **Verify notification settings in Karere:**
   - Open Preferences → Notifications
   - Ensure notifications are enabled

3. **Test system notifications:**
   ```bash
   notify-send "Test" "This is a test notification"
   ```

4. **Check Do Not Disturb status:**
   - Disable Do Not Disturb mode temporarily

5. **Flatpak permissions:**
   ```bash
   flatpak permission-set notifications io.github.tobagin.karere yes
   ```

### Notification Sound Issues

#### Issue: Notifications appear but no sound plays

**Solutions:**
1. **Check system sound settings:**
   - Ensure notification sounds are enabled in system settings
   - Test with other applications

2. **Verify Karere sound settings:**
   - Check Preferences → Notifications → Sounds

3. **Audio system:**
   ```bash
   # Test audio functionality
   speaker-test -t sine -f 1000 -l 1
   ```

4. **Flatpak audio permissions:**
   ```bash
   flatpak permission-set pulseaudio io.github.tobagin.karere yes
   ```

### Duplicate Notifications

#### Issue: Receiving duplicate notifications for same message

**Solutions:**
1. **Disable browser notifications:**
   - Ensure WhatsApp Web notifications are disabled in regular browsers
   - Close other WhatsApp Web sessions

2. **Check phone settings:**
   - Review WhatsApp notification settings on your phone
   - Avoid conflicts between phone and desktop notifications

3. **Clear notification history:**
   ```bash
   rm -rf ~/.var/app/io.github.tobagin.karere/data/notifications/*
   ```

## Theme and Appearance Issues

### Theme Not Applying

#### Issue: Karere doesn't follow system theme

**Solutions:**
1. **Check theme settings:**
   - Open Preferences → Appearance
   - Verify "System Theme" is selected

2. **Verify system theme:**
   ```bash
   gsettings get org.gnome.desktop.interface color-scheme
   ```

3. **Restart application:**
   - Close and reopen Karere after theme changes

4. **GTK theme compatibility:**
   - Ensure your GTK theme supports LibAdwaita

### Display and Scaling Issues

#### Issue: Interface appears too small or too large

**Solutions:**
1. **Check display scaling:**
   ```bash
   # Get current scaling factor
   gsettings get org.gnome.desktop.interface scaling-factor
   ```

2. **Adjust window size:**
   - Manually resize Karere window
   - Settings are remembered for future launches

3. **Font scaling:**
   ```bash
   # Check text scaling factor
   gsettings get org.gnome.desktop.interface text-scaling-factor
   ```

4. **High DPI display settings:**
   - Configure proper DPI settings for your display
   - Consider using fractional scaling

## Performance Issues

### High Memory Usage

#### Issue: Karere consuming too much RAM

**Solutions:**
1. **Monitor memory usage:**
   ```bash
   # Check Karere memory usage
   ps aux | grep karere
   top -p $(pgrep karere)
   ```

2. **Clear cache regularly:**
   ```bash
   rm -rf ~/.var/app/io.github.tobagin.karere/cache/*
   ```

3. **Reduce chat history:**
   - Clear chat history in WhatsApp Web settings
   - Limit media downloads

4. **Restart application periodically:**
   - Close and reopen Karere daily for heavy usage

### High CPU Usage

#### Issue: Karere causing high CPU load

**Solutions:**
1. **Identify CPU usage:**
   ```bash
   htop -p $(pgrep karere)
   ```

2. **Check for background processes:**
   - Look for unnecessary background activities
   - Monitor JavaScript performance in Developer Tools

3. **Reduce animations:**
   - Disable animations in GNOME settings
   - Use performance mode on laptops

4. **Update application:**
   - Ensure you're using the latest version with performance improvements

### Slow Performance

#### Issue: Application feels sluggish or unresponsive

**Solutions:**
1. **Check system resources:**
   ```bash
   free -h
   df -h
   ```

2. **Clear temporary files:**
   ```bash
   rm -rf ~/.var/app/io.github.tobagin.karere/cache/*
   rm -rf ~/.var/app/io.github.tobagin.karere/data/webkit/LocalStorage/*
   ```

3. **Reduce visual effects:**
   - Disable desktop animations
   - Use simpler themes

4. **Network performance:**
   ```bash
   # Test network speed
   speedtest-cli
   ```

## File Sharing Issues

### Cannot Send Files

#### Issue: File uploads fail or don't start

**Solutions:**
1. **Check file size limits:**
   - WhatsApp Web has file size limits (16MB for documents, 64MB for media)
   - Compress large files before sending

2. **File permissions:**
   ```bash
   # Check file permissions
   ls -la /path/to/file
   ```

3. **File type restrictions:**
   - Some file types may be blocked by WhatsApp
   - Try renaming file extension or compressing

4. **Network connectivity:**
   - Ensure stable internet connection for uploads

### Download Issues

#### Issue: Cannot download files from chats

**Solutions:**
1. **Check download location:**
   ```bash
   # Verify Downloads folder permissions
   ls -la ~/Downloads/
   ```

2. **Storage space:**
   ```bash
   df -h ~/Downloads/
   ```

3. **Flatpak permissions:**
   ```bash
   flatpak permission-set downloads io.github.tobagin.karere yes
   ```

4. **File associations:**
   - Configure default applications for file types

## Audio and Video Issues

### Call Problems

#### Issue: Cannot make or receive calls

WhatsApp calls in Karere use the browser-based calling system.

**Solutions:**
1. **Microphone permissions:**
   ```bash
   flatpak permission-set microphone io.github.tobagin.karere yes
   ```

2. **Camera permissions:**
   ```bash
   flatpak permission-set camera io.github.tobagin.karere yes
   ```

3. **Audio system:**
   ```bash
   # Test microphone
   arecord -d 5 test.wav && aplay test.wav
   ```

4. **Browser compatibility:**
   - WhatsApp Web calls may have limitations compared to mobile app

## Logging and Debugging

### Enabling Debug Logging

For troubleshooting complex issues:

1. **Enable debug logging:**
   - Open Preferences → Privacy → Logging
   - Enable "Debug Logging"

2. **Run with verbose output:**
   ```bash
   KARERE_DEBUG=1 flatpak run io.github.tobagin.karere
   ```

3. **Check log files:**
   ```bash
   ls -la ~/.var/app/io.github.tobagin.karere/data/logs/
   tail -f ~/.var/app/io.github.tobagin.karere/data/logs/karere.log
   ```

### Collecting Crash Information

If Karere crashes:

1. **Enable crash reporting:**
   - Open Preferences → Privacy → Crash Reporting
   - Enable reporting (helps developers fix issues)

2. **Manual crash information:**
   ```bash
   # Run with GDB for detailed crash info
   gdb --args flatpak run io.github.tobagin.karere
   (gdb) run
   # When crash occurs:
   (gdb) bt
   ```

3. **System logs:**
   ```bash
   journalctl --user -u app.flatpak.io.github.tobagin.karere
   ```

## Getting Additional Help

### Before Reporting Issues

1. **Search existing issues:**
   - Check [GitHub Issues](https://github.com/tobagin/karere-vala/issues)
   - Look for similar problems and solutions

2. **Gather information:**
   - Karere version: Help → About Karere
   - System information: `uname -a`
   - Desktop environment: `echo $XDG_CURRENT_DESKTOP`
   - Error logs and crash reports

3. **Try safe mode:**
   ```bash
   # Run with minimal configuration
   rm -rf ~/.var/app/io.github.tobagin.karere/config/
   flatpak run io.github.tobagin.karere
   ```

### Reporting Bugs

When reporting issues, please include:

1. **System Information:**
   - Operating system and version
   - Desktop environment
   - Karere version and installation method

2. **Steps to Reproduce:**
   - Detailed steps that trigger the issue
   - Expected vs. actual behavior

3. **Log Files:**
   - Relevant log entries
   - Crash reports (if applicable)

4. **Screenshots/Videos:**
   - Visual evidence of the problem
   - Screen recordings for complex issues

### Community Support

- **GitHub Discussions:** General questions and community support
- **GitHub Issues:** Bug reports and feature requests
- **Project Wiki:** Additional documentation and community tips
- **Matrix/Discord:** Real-time community chat (check project README)

---

*If your issue isn't covered here, check the [FAQ](../FAQ.md) or report it on [GitHub Issues](https://github.com/tobagin/karere-vala/issues).*
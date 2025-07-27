# Frequently Asked Questions (FAQ)

> Common questions and answers about Karere

## General Questions

### What is Karere?

**Q: What exactly is Karere and how is it different from using WhatsApp Web in a browser?**

A: Karere is a native GTK4/LibAdwaita application that wraps WhatsApp Web in a purpose-built desktop application. Unlike using WhatsApp Web in a regular browser, Karere provides:

- **Native desktop notifications** using your system's notification system
- **True theme integration** with GNOME and other GTK-based desktop environments
- **Comprehensive logging and debugging** capabilities for troubleshooting
- **Crash reporting system** to help improve stability
- **Optimized resource usage** compared to running in a browser tab
- **Native window management** with proper integration into your desktop workflow

### System Compatibility

**Q: What operating systems and desktop environments does Karere support?**

A: Karere is designed primarily for Linux desktop environments:

**Fully Supported:**
- GNOME (primary target with best integration)
- Pantheon (elementary OS)
- Budgie

**Good Compatibility:**
- Cinnamon
- MATE
- Unity

**Basic Functionality:**
- XFCE (limited theming)
- KDE Plasma (no theme integration)
- i3/Sway (window management only)

**System Requirements:**
- Linux distribution with GTK4 support
- GTK4 4.14 or later
- LibAdwaita 1.5 or later
- WebKitGTK 6.0

**Q: Can I run Karere on Windows or macOS?**

A: No, Karere is specifically designed for Linux desktop environments and uses Linux-specific technologies like Flatpak, GTK4, and LibAdwaita. For Windows and macOS, you can use WhatsApp Web in a browser or look for platform-specific alternatives.

### Installation and Setup

**Q: What's the recommended way to install Karere?**

A: The recommended installation method is Flatpak because it provides:
- Sandboxed security
- Automatic updates
- Consistent behavior across distributions
- No dependency conflicts

**Installation order of preference:**
1. **Flathub** (when available): `flatpak install flathub io.github.tobagin.karere`
2. **Development build**: Clone and build using `./scripts/build.sh --dev --install`
3. **Source build**: For advanced users and contributors

**Q: Why does Karere need so many permissions when installing via Flatpak?**

A: Karere requests these essential permissions:
- **Network access**: Required to connect to WhatsApp Web servers
- **Notifications**: For native desktop notifications
- **File system (Downloads)**: To save files shared through WhatsApp
- **Audio/Video**: For WhatsApp Web calls (optional, only when needed)

All permissions are necessary for core functionality and follow the principle of least privilege.

**Q: Can I install both the stable and development versions?**

A: Yes! Karere provides separate Flatpak applications:
- **Stable**: `io.github.tobagin.karere`
- **Development**: `io.github.tobagin.karere.Devel`

They can coexist on the same system with separate data directories and settings.

## Usage and Features

### WhatsApp Integration

**Q: Does Karere support all WhatsApp Web features?**

A: Yes, Karere provides complete access to all WhatsApp Web functionality because it renders the actual WhatsApp Web interface. This includes:
- Text messages, voice messages, and media sharing
- Group chats and broadcast lists
- Voice and video calls
- Status updates
- WhatsApp Business features
- All WhatsApp Web settings and preferences

**Q: Can I use multiple WhatsApp accounts with Karere?**

A: Each instance of Karere supports one WhatsApp account. However, you can:
- Run multiple Flatpak instances for different accounts (each with separate data)
- Use the development version alongside the stable version for two accounts
- Switch accounts by logging out and logging in with a different phone number

**Q: Does Karere work if my phone is offline?**

A: No, like WhatsApp Web, Karere requires your phone to be connected to the internet and have WhatsApp running. This is a WhatsApp Web limitation, not specific to Karere. Your phone acts as the primary device that relays messages to the web interface.

### Notifications

**Q: Why aren't I receiving notifications?**

A: Check these common causes:

1. **Notification permissions**: Ensure notifications are enabled in system settings
2. **Do Not Disturb**: Check if your desktop has Do Not Disturb enabled
3. **Karere settings**: Verify notifications are enabled in Preferences → Notifications
4. **Flatpak permissions**: Run `flatpak permission-set notifications io.github.tobagin.karere yes`
5. **System notification service**: Ensure your desktop's notification daemon is running

**Q: Can I customize notification behavior?**

A: Yes, Karere provides several notification options:
- Enable/disable all notifications
- Control notification sounds
- Show/hide message previews
- Configure Do Not Disturb integration
- Set priority for different chat types

Access these settings through **Preferences → Notifications**.

**Q: Why do I see duplicate notifications?**

A: This usually happens when:
- WhatsApp Web is also open in a browser
- Multiple Karere instances are running
- Phone notifications aren't properly synchronized

**Solution**: Close other WhatsApp Web sessions and ensure only one Karere instance is running.

### Performance and Resources

**Q: How much memory and CPU does Karere use?**

A: Typical resource usage:
- **Memory**: 200-800MB (depending on usage)
- **CPU**: 1-5% when idle, 10-30% during active use
- **Storage**: 50-200MB for application data and cache

This is generally more efficient than keeping WhatsApp Web open in a browser tab alongside other browser tabs.

**Q: Why does Karere use so much memory?**

A: Memory usage comes from:
- WebKit rendering engine (~100-300MB)
- WhatsApp Web content and cached data (~50-200MB)
- Karere application itself (~20-50MB)

This is normal for a web-based application. To reduce memory usage:
- Clear cache regularly
- Restart Karere daily
- Close when not actively using WhatsApp

## Privacy and Security

### Data Privacy

**Q: What data does Karere collect or store?**

A: Karere follows a privacy-first approach:

**Data Stored Locally:**
- Application preferences and settings
- WhatsApp Web session data (cookies, local storage)
- Application logs (optional, can be disabled)
- Crash reports (only if you opt-in)

**Data NOT Collected:**
- Your WhatsApp messages or media
- Personal information or contacts
- Usage analytics or telemetry
- Network traffic content

**Q: Does Karere send data to external servers?**

A: Karere only communicates with:
- **WhatsApp servers**: For normal WhatsApp Web functionality
- **Crash reporting service**: Only if you explicitly opt-in and submit reports

No analytics, telemetry, or tracking data is transmitted.

**Q: Is the crash reporting safe to enable?**

A: Yes, the crash reporting system is designed with privacy in mind:
- **Opt-in only**: Disabled by default, requires explicit user consent
- **Anonymous data**: No personal information included
- **Local storage**: All crash data stored locally until you decide to submit
- **User review**: You can review crash data before submission
- **Revocable**: Can be disabled at any time

### Security

**Q: Is Karere secure?**

A: Karere implements several security measures:
- **Flatpak sandboxing**: Isolates the application from the rest of your system
- **HTTPS only**: All communication encrypted in transit
- **No external tracking**: No analytics or third-party scripts
- **Regular updates**: Security patches through Flatpak updates
- **Minimal permissions**: Only requests necessary system access

**Q: How does Karere handle WhatsApp's end-to-end encryption?**

A: Karere doesn't interfere with WhatsApp's encryption. Since it renders the actual WhatsApp Web interface, all encryption and decryption happens exactly as it would in a browser. Your messages remain end-to-end encrypted between you and your contacts.

## Troubleshooting

### Common Issues

**Q: Karere won't start or shows a blank window**

A: Try these solutions in order:
1. Check internet connectivity
2. Clear application cache: `rm -rf ~/.var/app/io.github.tobagin.karere/cache/*`
3. Verify WebKitGTK installation: `flatpak info org.gnome.Platform`
4. Check system logs: `journalctl --user -f | grep karere`
5. Restart with debug logging: `KARERE_DEBUG=1 flatpak run io.github.tobagin.karere`

**Q: WhatsApp Web shows "Could not connect" error**

A: This is usually a network connectivity issue:
1. Test WhatsApp Web in a regular browser
2. Check firewall settings
3. Verify DNS resolution: `nslookup web.whatsapp.com`
4. Test with different network connection
5. Check for corporate proxy/firewall restrictions

**Q: The application crashes frequently**

A: For frequent crashes:
1. Enable crash reporting to help developers identify the issue
2. Update to the latest version
3. Check system logs for error messages
4. Clear application data: `rm -rf ~/.var/app/io.github.tobagin.karere/data/*`
5. Report the issue on GitHub with crash logs

### Performance Issues

**Q: Karere is slow or uses too many resources**

A: Optimize performance with these steps:
1. **Clear cache regularly**: `rm -rf ~/.var/app/io.github.tobagin.karere/cache/*`
2. **Restart daily**: Close and reopen Karere once per day
3. **Monitor system resources**: Use `htop` to check available memory
4. **Reduce visual effects**: Disable desktop animations
5. **Update system**: Ensure you have the latest graphics drivers

**Q: High CPU usage even when idle**

A: High idle CPU usage may indicate:
- Background JavaScript activity in WhatsApp Web
- System graphics issues
- Outdated WebKit or graphics drivers

**Solutions**:
- Update your system and graphics drivers
- Monitor CPU usage with `top -p $(pgrep karere)`
- Report persistent high CPU usage as a performance issue

## Development and Contributing

### Development Questions

**Q: Can I contribute to Karere development?**

A: Absolutely! Karere welcomes contributions:
- **Code contributions**: Submit pull requests on GitHub
- **Bug reports**: Report issues with detailed information
- **Feature requests**: Suggest new functionality
- **Documentation**: Help improve guides and documentation
- **Translation**: Contribute translations for your language
- **Testing**: Test development versions and provide feedback

**Q: How do I build Karere from source?**

A: For development builds:
```bash
git clone https://github.com/tobagin/karere-vala.git
cd karere-vala
./scripts/build.sh --dev --install
```

For native builds (advanced):
```bash
meson setup build --prefix=/usr/local
meson compile -C build
sudo meson install -C build
```

See the [development documentation](development/building.md) for detailed instructions.

**Q: What programming language is Karere written in?**

A: Karere is written in:
- **Vala**: Primary application language
- **Blueprint**: UI definition language (compiles to GTK XML)
- **Meson**: Build system
- **Shell scripts**: Build and utility scripts

### Technical Questions

**Q: Why Vala instead of other languages?**

A: Vala was chosen because:
- Native compilation for optimal performance
- Excellent GTK/GNOME integration
- Memory safety with garbage collection
- Familiar C#/Java-like syntax
- Direct access to GObject and GTK APIs
- Small runtime footprint

**Q: Can I create plugins or extensions for Karere?**

A: Currently, Karere doesn't have a plugin system, but this is being considered for future versions. For now, you can:
- Fork the project and add custom features
- Submit feature requests for functionality you need
- Contribute to the main codebase

## Comparison with Alternatives

### Vs. Browser-based WhatsApp Web

**Q: Why use Karere instead of WhatsApp Web in a browser?**

A: Karere advantages:
- **Native notifications**: Better integration with desktop notifications
- **Theme integration**: Follows your system theme automatically
- **Resource efficiency**: More efficient than browser + tabs
- **Window management**: Proper desktop application behavior
- **Logging/debugging**: Comprehensive troubleshooting capabilities
- **Offline indicator**: Clear indication when connection is lost

### Vs. Other WhatsApp Desktop Clients

**Q: How does Karere compare to other WhatsApp desktop applications?**

A: Karere differentiators:
- **Native Linux integration**: Built specifically for Linux desktop environments
- **LibAdwaita design**: Modern GNOME-style interface
- **Comprehensive logging**: Detailed debugging and troubleshooting features
- **Privacy-focused**: No analytics or tracking
- **Open source**: Transparent development and community-driven
- **Flatpak distribution**: Secure, sandboxed installation

## Future Development

### Roadmap

**Q: What features are planned for future releases?**

A: Check the [product roadmap](../.agent-os/product/roadmap.md) for detailed plans. Upcoming features may include:
- Enhanced notification actions
- Better keyboard shortcuts
- Improved performance optimizations
- Additional privacy controls
- Plugin system (under consideration)

**Q: How often is Karere updated?**

A: Update frequency depends on the release channel:
- **Development builds**: Updated with each commit/merge
- **Beta releases**: Monthly feature releases
- **Stable releases**: Quarterly major releases with regular bugfix updates

## Getting Help

### Support Channels

**Q: Where can I get help with Karere?**

A: Support options:
1. **Documentation**: Check [user guides](user-guide/) and [troubleshooting](troubleshooting/)
2. **FAQ**: This document for common questions
3. **GitHub Issues**: Report bugs and technical issues
4. **GitHub Discussions**: Community support and general questions
5. **Community Chat**: Real-time support (check project README for links)

**Q: How do I report a bug effectively?**

A: Include this information in bug reports:
1. **System details**: OS, desktop environment, Karere version
2. **Steps to reproduce**: Detailed reproduction steps
3. **Expected vs actual behavior**: What should happen vs what happens
4. **Logs and screenshots**: Error messages and visual evidence
5. **Consistency**: Whether the issue happens every time

### Community

**Q: How can I stay updated on Karere development?**

A: Follow these channels:
- **GitHub repository**: Watch for releases and updates
- **GitHub Discussions**: Community announcements
- **Project blog/website**: Major updates and news
- **Social media**: Follow project accounts (check README)

---

*Don't see your question here? Check the [user guide](user-guide/) or ask in [GitHub Discussions](https://github.com/tobagin/karere-vala/discussions).*
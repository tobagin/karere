# Build Versions Documentation

## Overview

Karere supports two distinct build configurations: **Production (default)** and **Development**. These versions serve different purposes and have specific characteristics that affect the application's behavior, permissions, and intended use cases.

## Version Configurations

### Production Version (Default)

**App ID**: `io.github.tobagin.karere`  
**Display Name**: `Karere`  
**Profile**: `default`

#### Build Configuration
- **Meson Profile**: `default` (meson_options.txt:2)
- **Tests**: Disabled (`-Dtests=false`)
- **Vala Preprocessor**: No `DEVELOPMENT` flag
- **Manifest**: `packaging/io.github.tobagin.karere.yml`

#### Features & Permissions
- **Flatpak Permissions**:
  - Network access (`--share=network`)
  - IPC access (`--share=ipc`) 
  - Wayland/X11 display (`--socket=wayland`, `--socket=fallback-x11`)
  - Audio support (`--socket=pulseaudio`)
  - Desktop notifications (`--talk-name=org.freedesktop.Notifications`)
  - Downloads access (`--filesystem=xdg-download`)
  - Hardware acceleration (`--device=dri`)

- **Metainfo**: Includes internationalization support
- **Application Data**: Standard application data directory
- **Debugging**: Limited debugging capabilities
- **Build Source**: Git tag (stable releases)

#### Use Cases
- End-user installations
- Production deployments
- Stable, tested features only
- Distribution through Flathub
- General public use

---

### Development Version

**App ID**: `io.github.tobagin.karere.Devel`  
**Display Name**: `Karere (Devel)`  
**Profile**: `development`

#### Build Configuration
- **Meson Profile**: `development` (meson_options.txt:2)
- **Tests**: Enabled (`-Dtests=true`)
- **Vala Preprocessor**: `DEVELOPMENT` flag enabled (meson.build:69)
- **Manifest**: `packaging/io.github.tobagin.karere.Devel.yml`

#### Features & Permissions
- **Flatpak Permissions** (inherits production permissions plus):
  - Separate data directory (`--persist=.var/app/io.github.tobagin.karere.Devel`)
  - Extended filesystem access:
    - Documents (`--filesystem=xdg-documents`)
    - Music (`--filesystem=xdg-music`)
    - Videos (`--filesystem=xdg-videos`)
    - Pictures (`--filesystem=xdg-pictures`)
  - Development permissions (`--allow=devel`)
  - Debug environment (`--env=G_MESSAGES_DEBUG=all`)

- **Metainfo**: No internationalization (skips i18n to avoid ITS rules issues)
- **Application Data**: Isolated from production version
- **Debugging**: Full debugging capabilities enabled
- **Build Source**: Local directory (`path: ..`)

#### Use Cases
- Active development and testing
- Feature development and debugging
- Side-by-side installation with production
- Testing new features before release
- Developer workflow and CI/CD

---

## Technical Implementation

### Meson Build System

The build system uses the `profile` option to control compilation:

```meson
# meson_options.txt
option('profile', type: 'combo', choices: ['default', 'development'], value: 'default', description: 'Build profile')
```

#### Profile-Specific Logic (meson.build:13-19)

```meson
if get_option('profile') == 'development'
    app_id = 'io.github.tobagin.karere.Devel'
    app_name = 'Karere (Devel)'
else
    app_id = 'io.github.tobagin.karere'
    app_name = 'Karere'
endif
```

#### Vala Compilation Flags (meson.build:67-70)

```meson
# Add development flag for preprocessor directives
if get_option('profile') == 'development'
    vala_args += ['-D', 'DEVELOPMENT']
endif
```

#### Metainfo Handling (meson.build:234-257)

```meson
# Skip i18n for development builds to avoid ITS rules issues
if get_option('profile') == 'development'
    metainfo_file = configure_file(...)  # Direct configuration
else
    metainfo_file = i18n.merge_file(...)  # With internationalization
endif
```

### Configuration Template System

The build system uses template files with configuration substitution:

#### Generated Files
- **config.vala**: Contains app constants (meson.build:52-56)
- **preferences.blp**: UI preferences with app-specific IDs (meson.build:87-92)
- **resources.xml**: Resource definitions (meson.build:141-146)
- **Desktop files**: Application launchers (meson.build:208-222)
- **Metainfo files**: Application metadata (meson.build:234-257)
- **GSchema files**: Settings schema (meson.build:269-274)

#### Dynamic Icon Handling (meson.build:127-139)

Icons are copied with app-specific naming to support both versions:
- Production: `io.github.tobagin.karere-*`
- Development: `io.github.tobagin.karere.Devel-*`

---

## Build Workflows

### Using the Build Script

The project includes a unified build script at `scripts/build.sh`:

#### Production Build
```bash
./scripts/build.sh
# or
./scripts/build.sh --help  # for usage information
```

#### Development Build
```bash
./scripts/build.sh --dev
```

#### Script Behavior
- **Default**: Builds production version using `packaging/io.github.tobagin.karere.yml`
- **With --dev**: Builds development version using `packaging/io.github.tobagin.karere.Devel.yml`
- **Always installs**: Both versions are automatically installed after building
- **Force clean**: Always performs clean builds (`--force-clean`)

### Manual Flatpak Building

#### Production Version
```bash
flatpak-builder --force-clean --user --install --install-deps-from=flathub build packaging/io.github.tobagin.karere.yml
```

#### Development Version
```bash
flatpak-builder --force-clean --user --install --install-deps-from=flathub build packaging/io.github.tobagin.karere.Devel.yml
```

### Running the Applications

#### Production Version
```bash
flatpak run io.github.tobagin.karere
```

#### Development Version
```bash
flatpak run io.github.tobagin.karere.Devel
```

---

## Data Separation

### Production Data Location
- **Flatpak**: `~/.var/app/io.github.tobagin.karere/`
- **GSettings Schema**: `io.github.tobagin.karere`
- **Desktop File**: `io.github.tobagin.karere.desktop`

### Development Data Location
- **Flatpak**: `~/.var/app/io.github.tobagin.karere.Devel/`
- **GSettings Schema**: `io.github.tobagin.karere.Devel`
- **Desktop File**: `io.github.tobagin.karere.Devel.desktop`

This separation allows both versions to:
- Run simultaneously without conflicts
- Maintain independent settings and data
- Have separate application entries in desktop environments
- Use different notification systems and integrations

---

## Development Features

### Compile-Time Conditionals

The development version enables the `DEVELOPMENT` preprocessor flag, which can be used in Vala code for:
- Debug-only code paths
- Development-specific features
- Enhanced logging
- Testing utilities

### Enhanced Debugging

Development builds include:
- **G_MESSAGES_DEBUG=all**: Comprehensive debug output
- **Test Suite**: Unit and integration tests enabled
- **Extended Permissions**: Access to more system resources for testing
- **Development Sandbox**: `--allow=devel` for additional capabilities

### Testing Integration

The development profile enables comprehensive testing:
- **Unit Tests**: Vala-based test suite (tests/*)
- **Build Configuration Tests**: Python-based validation (tests/test_build_config.py)
- **Validation Tests**: Desktop file and metainfo validation

---

## Best Practices

### For End Users
- Use the **production version** for daily use
- Install the **development version** only for testing new features
- Both versions can coexist without conflicts

### For Developers
- Use the **development version** for active development
- Test changes in the development version before releasing
- Use the production version to verify release candidates
- Leverage the separate data directories for testing different configurations

### For Contributors
- Build with `--dev` flag during development
- Run tests using the development profile
- Ensure changes work in both production and development builds
- Use the enhanced debugging capabilities in development builds

---

## Version Identification

### At Runtime
The application can determine its build type through:
```vala
// In development builds, this preprocessor flag is available
#if DEVELOPMENT
    // Development-specific code
#endif
```

### Configuration Constants
```vala
// Generated in config.vala
public const string APP_ID = "io.github.tobagin.karere[.Devel]";
public const string APP_NAME = "Karere[ (Devel)]";
```

### Build Information
- **Version**: Consistent across both builds (from meson.project_version())
- **App ID**: Distinguishes between production and development
- **Display Name**: Visual indication of build type
- **Data Paths**: Automatically isolated by Flatpak based on App ID

---

## Troubleshooting

### Common Issues

#### Build Conflicts
- **Problem**: Build artifacts conflict between versions
- **Solution**: The build script uses `--force-clean` to prevent this

#### Data Confusion
- **Problem**: Unsure which version's data is being used
- **Solution**: Check the App ID in About dialog or use `flatpak ps`

#### Permission Issues
- **Problem**: Development version has different capabilities
- **Solution**: Review the extended permissions in the development manifest

### Verification Commands

#### Check Installed Versions
```bash
flatpak list | grep karere
```

#### Check Running Processes
```bash
flatpak ps
```

#### View Application Info
```bash
flatpak info io.github.tobagin.karere
flatpak info io.github.tobagin.karere.Devel
```

---

This documentation provides a complete reference for understanding and working with both production and development versions of Karere. The separation ensures a stable user experience while enabling active development and testing.
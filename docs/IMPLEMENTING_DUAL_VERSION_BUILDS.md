# How to Implement Dual-Version Build Systems

## Overview

This guide explains how to implement a dual-version build system like Karere's, enabling both **Production** and **Development** versions of your application to coexist. This pattern is particularly valuable for desktop applications distributed via Flatpak, allowing stable user deployments alongside active development.

## Prerequisites

- **Meson build system** (≥ 1.0.0)
- **Flatpak** for packaging
- **Template-based configuration files** (.in files)
- Understanding of preprocessor directives in your language

## Architecture Overview

The dual-version system works through:
1. **Build Profile Selection**: Meson option to choose `default` vs `development`
2. **Conditional Configuration**: Different App IDs, names, and permissions
3. **Template System**: Dynamic file generation based on profile
4. **Preprocessor Flags**: Compile-time conditionals for development features
5. **Isolated Data**: Separate application data directories

---

## Step 1: Meson Configuration Setup

### 1.1 Create Build Profile Option

Create or modify `meson_options.txt`:

```meson
option('profile', type: 'combo', choices: ['default', 'development'], value: 'default', description: 'Build profile')
option('tests', type: 'boolean', value: true, description: 'Enable tests')
```

### 1.2 Configure Conditional Build Logic

Add to your `meson.build` (adapt App IDs to your project):

```meson
# Profile-based configuration
if get_option('profile') == 'development'
    app_id = 'com.yourorg.yourapp.Devel'
    app_name = 'YourApp (Devel)'
else
    app_id = 'com.yourorg.yourapp'
    app_name = 'YourApp'
endif

# Configuration data for templates
conf_data = configuration_data()
conf_data.set('APP_ID', app_id)
conf_data.set('APP_NAME', app_name)
conf_data.set('VERSION', meson.project_version())
conf_data.set('GETTEXT_PACKAGE', meson.project_name())
conf_data.set('LOCALEDIR', get_option('prefix') / get_option('localedir'))
conf_data.set('APP_PATH', app_id.replace('.', '/'))
```

### 1.3 Add Development Preprocessor Flags

For Vala projects:
```meson
# Vala arguments
vala_args = [
    '--target-glib=2.78',
    '--pkg=posix',
]

# Add development flag for preprocessor directives
if get_option('profile') == 'development'
    vala_args += ['-D', 'DEVELOPMENT']
endif
```

For C/C++ projects:
```meson
c_args = []
if get_option('profile') == 'development'
    c_args += ['-DDEVELOPMENT']
endif
```

For other languages, adjust accordingly.

---

## Step 2: Template File System

### 2.1 Create Configuration Template

Create `src/config.[language].in` (e.g., `src/config.vala.in`):

```vala
namespace Config {
    public const string APP_ID = "@APP_ID@";
    public const string APP_NAME = "@APP_NAME@";
    public const string VERSION = "@VERSION@";
    public const string GETTEXT_PACKAGE = "@GETTEXT_PACKAGE@";
    public const string LOCALEDIR = "@LOCALEDIR@";
}
```

### 2.2 Generate Configuration in Meson

```meson
# Generate dynamic config file
config_file = configure_file(
    input: 'src/config.vala.in',
    output: 'config.vala',
    configuration: conf_data
)
```

### 2.3 Create Desktop File Template

Create `data/com.yourorg.yourapp.desktop.in`:

```desktop
[Desktop Entry]
Type=Application
Name=@APP_NAME@
GenericName=Your Application
Comment=Description of your application
Exec=@APP_ID@
Icon=@APP_ID@
StartupNotify=true
Categories=Network;InstantMessaging;
Keywords=keyword1;keyword2;
```

Generate desktop file:
```meson
desktop_configured = configure_file(
    input: 'data/com.yourorg.yourapp.desktop.in',
    output: app_id + '.desktop.in.configured',
    configuration: conf_data
)

desktop_file = i18n.merge_file(
    input: desktop_configured,
    output: app_id + '.desktop',
    type: 'desktop',
    po_dir: 'po',
    install: true,
    install_dir: get_option('datadir') / 'applications'
)
```

### 2.4 Create GSchema Template

Create `data/com.yourorg.yourapp.gschema.xml.in`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<schemalist>
  <schema path="/@APP_PATH@/" id="@APP_ID@">
    <key name="window-width" type="i">
      <default>1200</default>
      <summary>Window width</summary>
      <description>The default width of the main window</description>
    </key>
    <key name="window-height" type="i">
      <default>800</default>
      <summary>Window height</summary>
      <description>The default height of the main window</description>
    </key>
    <!-- Add your application-specific settings -->
  </schema>
</schemalist>
```

Generate GSchema:
```meson
gschema_file = configure_file(
    input: 'data/com.yourorg.yourapp.gschema.xml.in',
    output: app_id + '.gschema.xml',
    configuration: conf_data
)

install_data(
    gschema_file,
    install_dir: get_option('datadir') / 'glib-2.0' / 'schemas'
)
```

### 2.5 Create MetaInfo Template

Create `data/com.yourorg.yourapp.metainfo.xml.in`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<component type="desktop">
  <id>@APP_ID@</id>
  <metadata_license>CC0-1.0</metadata_license>
  <project_license>GPL-3.0-or-later</project_license>
  <name>@APP_NAME@</name>
  <summary>Brief description of your application</summary>
  <description>
    <p>Detailed description of your application...</p>
  </description>
  <launchable type="desktop-id">@APP_ID@.desktop</launchable>
  <!-- Add screenshots, releases, etc. -->
</component>
```

Handle metainfo conditionally:
```meson
# Skip i18n for development builds to avoid ITS rules issues
if get_option('profile') == 'development'
    metainfo_file = configure_file(
        input: 'data/com.yourorg.yourapp.metainfo.xml.in',
        output: app_id + '.metainfo.xml',
        configuration: conf_data,
        install: true,
        install_dir: get_option('datadir') / 'metainfo'
    )
else
    metainfo_configured = configure_file(
        input: 'data/com.yourorg.yourapp.metainfo.xml.in',
        output: app_id + '.metainfo.xml.in',
        configuration: conf_data
    )
    
    metainfo_file = i18n.merge_file(
        input: metainfo_configured,
        output: app_id + '.metainfo.xml',
        po_dir: 'po',
        install: true,
        install_dir: get_option('datadir') / 'metainfo'
    )
endif
```

---

## Step 3: Flatpak Manifests

### 3.1 Production Manifest

Create `packaging/com.yourorg.yourapp.yml`:

```yaml
app-id: com.yourorg.yourapp
runtime: org.gnome.Platform
runtime-version: '48'
sdk: org.gnome.Sdk
command: yourapp

finish-args:
  # Basic permissions
  - --share=network
  - --share=ipc
  - --socket=wayland
  - --socket=fallback-x11
  - --socket=pulseaudio
  - --talk-name=org.freedesktop.Notifications
  - --filesystem=xdg-download
  - --device=dri

modules:
  # Your dependencies here
  
  - name: yourapp
    buildsystem: meson
    config-opts:
      - -Dtests=false
      - -Dprofile=default
    sources:
      - type: git
        url: https://github.com/yourusername/yourapp.git
        tag: v1.0.0
        commit: your-commit-hash
```

### 3.2 Development Manifest

Create `packaging/com.yourorg.yourapp.Devel.yml`:

```yaml
app-id: com.yourorg.yourapp.Devel
runtime: org.gnome.Platform
runtime-version: '48'
sdk: org.gnome.Sdk
command: yourapp

finish-args:
  # Inherit production permissions
  - --share=network
  - --share=ipc
  - --socket=wayland
  - --socket=fallback-x11
  - --socket=pulseaudio
  - --talk-name=org.freedesktop.Notifications
  - --device=dri
  
  # Development-specific permissions
  - --persist=.var/app/com.yourorg.yourapp.Devel
  - --filesystem=xdg-download
  - --filesystem=xdg-documents
  - --filesystem=xdg-music
  - --filesystem=xdg-videos
  - --filesystem=xdg-pictures
  - --allow=devel
  - --env=G_MESSAGES_DEBUG=all

modules:
  # Your dependencies here (same as production)
  
  - name: yourapp
    buildsystem: meson
    config-opts:
      - -Dtests=true
      - -Dprofile=development
    sources:
      - type: dir
        path: ..  # Build from local directory
```

---

## Step 4: Build Script

Create `scripts/build.sh`:

```bash
#!/bin/bash

# Build script for dual-version system
# Usage: ./build.sh [--dev]

set -e

# Default to production build
BUILD_TYPE="prod"

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --dev)
            BUILD_TYPE="dev"
            shift
            ;;
        --help)
            echo "Usage: $0 [--dev]"
            echo "  --dev      Build development version"
            echo "Default: Build production version"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

# Set manifest and app ID based on build type
if [ "$BUILD_TYPE" = "dev" ]; then
    MANIFEST="packaging/com.yourorg.yourapp.Devel.yml"
    APP_ID="com.yourorg.yourapp.Devel"
    echo "Building development version..."
else
    MANIFEST="packaging/com.yourorg.yourapp.yml"
    APP_ID="com.yourorg.yourapp"
    echo "Building production version..."
fi

BUILD_DIR="build"

echo "Using manifest: $MANIFEST"
echo "Build directory: $BUILD_DIR"

# Build and install
echo "Running flatpak-builder..."
flatpak-builder --force-clean --user --install --install-deps-from=flathub "$BUILD_DIR" "$MANIFEST"

echo "Build and installation complete!"
echo "Run with: flatpak run $APP_ID"
```

Make it executable:
```bash
chmod +x scripts/build.sh
```

---

## Step 5: Code-Level Integration

### 5.1 Using Preprocessor Directives

In your source code, use development-specific features:

**Vala example:**
```vala
public class Application : Adw.Application {
    public Application() {
        Object(
            application_id: Config.APP_ID,
            flags: ApplicationFlags.DEFAULT_FLAGS
        );
        
#if DEVELOPMENT
        // Development-only features
        this.flags |= ApplicationFlags.HANDLES_COMMAND_LINE;
        debug("Development mode enabled");
#endif
    }
    
#if DEVELOPMENT
    protected override int handle_local_options(VariantDict options) {
        if (options.contains("debug")) {
            Environment.set_variable("G_MESSAGES_DEBUG", "all", true);
        }
        return -1;
    }
#endif
}
```

**C example:**
```c
int main(int argc, char *argv[]) {
    GApplication *app = g_application_new(APP_ID, G_APPLICATION_DEFAULT_FLAGS);
    
#ifdef DEVELOPMENT
    g_application_set_flags(app, G_APPLICATION_HANDLES_COMMAND_LINE);
    g_debug("Development mode enabled");
#endif
    
    return g_application_run(app, argc, argv);
}
```

### 5.2 Runtime Detection

Create utility functions to detect build type:

```vala
namespace BuildInfo {
    public static bool is_development_build() {
#if DEVELOPMENT
        return true;
#else
        return false;
#endif
    }
    
    public static string get_build_type() {
        return is_development_build() ? "Development" : "Production";
    }
}
```

---

## Step 6: Testing Configuration

### 6.1 Conditional Test Building

In your `meson.build`:

```meson
# Tests (only in development or when explicitly enabled)
if get_option('tests')
    test_sources = files(
        'tests/test_application.vala',
        'tests/test_main.vala',
        # Add your test files
    )
    
    test_executable = executable(
        'test-runner',
        test_sources,
        # Include your main sources (excluding main.vala to avoid conflicts)
        config_file,
        dependencies: [
            # Your dependencies
        ],
        vala_args: vala_args,
        c_args: c_args,
        install: false
    )
    
    test(
        'unit-tests',
        test_executable,
        env: environment({'G_TEST_SRCDIR': meson.current_source_dir()})
    )
endif
```

---

## Step 7: Icon and Resource Handling

### 7.1 Dynamic Icon Names

Handle icons that need app-specific names:

```meson
# App-specific icon copying
icon_files = [
    'notification-symbolic.svg',
    'status-symbolic.svg'
]

icon_copies = []
foreach icon : icon_files
    base_name = icon.replace('-symbolic.svg', '')
    icon_copy = custom_target(
        'icon_' + base_name,
        input: 'data/icons/hicolor/symbolic/apps/' + 'com.yourorg.yourapp-' + icon,
        output: app_id + '-' + icon,
        command: ['cp', '@INPUT@', '@OUTPUT@']
    )
    icon_copies += icon_copy
endforeach

# Install icons with app-specific names
foreach icon : icon_files
    install_data(
        'data/icons/hicolor/symbolic/apps/' + 'com.yourorg.yourapp-' + icon,
        install_dir: get_option('datadir') / 'icons' / 'hicolor' / 'symbolic' / 'apps',
        rename: icon.replace('com.yourorg.yourapp', app_id)
    )
endforeach
```

### 7.2 Resources with App-Specific References

If using GResource, create `data/resources.xml.in`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<gresources>
  <gresource prefix="/@APP_PATH@/">
    <file compressed="true">ui/window.ui</file>
    <file compressed="true">ui/preferences.ui</file>
    <!-- App-specific icons -->
    <file>@APP_ID@-notification-symbolic.svg</file>
    <file>@APP_ID@-status-symbolic.svg</file>
  </gresource>
</gresources>
```

Generate resources:
```meson
resources_xml = configure_file(
    input: 'data/resources.xml.in',
    output: 'resources.xml',
    configuration: conf_data
)

resources = gnome.compile_resources(
    'resources',
    resources_xml,
    source_dir: [meson.current_build_dir(), 'data'],
    dependencies: icon_copies,  # Ensure icons are generated first
    c_name: 'yourapp'
)
```

---

## Step 8: UI Templates (Optional)

For UI files that reference app-specific resources, create templates:

Create `data/ui/preferences.blp.in` (Blueprint example):
```blueprint
using Gtk 4.0;
using Adw 1;

template $YourAppPreferences : Adw.PreferencesDialog {
    // Your UI content
    
    Adw.PreferencesPage notifications_page {
        title: _("Notifications");
        icon-name: "@APP_ID@-notification-symbolic";
        
        // Page content
    }
}
```

Generate UI files:
```meson
preferences_blp = configure_file(
    input: 'data/ui/preferences.blp.in',
    output: 'preferences.blp',
    configuration: conf_data
)

preferences_ui = custom_target(
    'preferences.ui',
    input: preferences_blp,
    output: 'preferences.ui',
    command: [blueprint_compiler, 'compile', '--output', '@OUTPUT@', '@INPUT@']
)
```

---

## Best Practices

### 1. Data Separation
- Always use different App IDs for production and development
- Leverage Flatpak's automatic data isolation
- Test both versions simultaneously

### 2. Permission Management
- Start with minimal permissions in production
- Add development-specific permissions only to development manifest
- Document permission differences

### 3. Build System
- Keep build logic simple and readable
- Use meaningful variable names (app_id, app_name)
- Test both build profiles regularly

### 4. Development Features
- Use preprocessor directives sparingly
- Focus on debugging and testing enhancements
- Avoid feature flags that could break production

### 5. Documentation
- Document all template variables
- Explain permission differences
- Provide clear build instructions

### 6. Version Management
- Use the same version number for both builds
- Differentiate through App ID and display name
- Consider separate Git branches for major development

---

## Testing the Implementation

### 1. Build Both Versions
```bash
# Production
./scripts/build.sh

# Development  
./scripts/build.sh --dev
```

### 2. Verify Separation
```bash
# Check both are installed
flatpak list | grep yourapp

# Run simultaneously
flatpak run com.yourorg.yourapp &
flatpak run com.yourorg.yourapp.Devel &
```

### 3. Validate Data Isolation
```bash
# Check data directories
ls ~/.var/app/com.yourorg.yourapp/
ls ~/.var/app/com.yourorg.yourapp.Devel/
```

### 4. Test Development Features
- Verify preprocessor flags work
- Check enhanced debugging
- Test development-specific permissions

---

## Troubleshooting

### Common Issues

1. **Template Variable Not Found**
   - Ensure variable is defined in `conf_data`
   - Check spelling in template files

2. **Icon Not Found**
   - Verify icon copying in build system
   - Check app-specific icon names match references

3. **Data Directory Conflicts**
   - Confirm App IDs are different
   - Check Flatpak manifest app-id values

4. **Build Failures**
   - Test both profiles independently
   - Verify all template files exist

### Debug Commands
```bash
# Check Flatpak app info
flatpak info com.yourorg.yourapp.Devel

# Inspect build directory
ls -la build/files/

# Check generated files
cat build/com.yourorg.yourapp.Devel.desktop
```

---

## Conclusion

This dual-version system provides:
- **User Safety**: Stable production builds for daily use
- **Developer Productivity**: Feature-rich development environment
- **Data Isolation**: No conflicts between versions
- **Flexibility**: Easy switching between build types
- **Professional Workflow**: Supports CI/CD and testing

The pattern scales well and can be adapted to various build systems and packaging formats beyond Meson and Flatpak.
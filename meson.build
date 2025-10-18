project(
    'karere',
    'vala', 'c',
    version: '0.9.4',
    meson_version: '>=0.60.0',
    default_options: [
        'warning_level=1',
        'werror=false',
    ],
    license: 'GPL-3.0-or-later'
)

if get_option('profile') == 'development'
    app_id = 'io.github.tobagin.karere.Devel'
    app_name = 'Karere (Devel)'
else
    app_id = 'io.github.tobagin.karere'
    app_name = 'Karere'
endif

# Import modules
gnome = import('gnome')
i18n = import('i18n')

# Dependencies
gtk_dep = dependency('gtk4', version: '>=4.10.0')
adwaita_dep = dependency('libadwaita-1', version: '>=1.8.0')
webkit_dep = dependency('webkitgtk-6.0', version: '>=2.40.0')
gio_dep = dependency('gio-2.0', version: '>=2.70.0')
glib_dep = dependency('glib-2.0', version: '>=2.70.0')

# Build configuration
cc = meson.get_compiler('c')
valac = meson.get_compiler('vala')
math_dep = cc.find_library('m', required: false)

# Configuration data
conf_data = configuration_data()
# For config.vala template (which includes quotes in the template)
conf_data.set('APP_ID', app_id)
conf_data.set('APP_NAME', app_name)
conf_data.set('VERSION', meson.project_version())
conf_data.set('GETTEXT_PACKAGE', meson.project_name())
conf_data.set('LOCALEDIR', get_option('prefix') / get_option('localedir'))
# For GSchema path (convert dots to slashes)
conf_data.set('APP_PATH', app_id.replace('.', '/'))

# Generate dynamic config file
config_vala = configure_file(
    input: 'src/Config.vala.in',
    output: 'Config.vala',
    configuration: conf_data
)

# No more template files - using preprocessor directives instead


# Vala arguments
vala_args = [
    '--target-glib=2.78',
    '--pkg=posix',
]

# Add development flag for preprocessor directives
if get_option('profile') == 'development'
    vala_args += ['-D', 'DEVELOPMENT']
endif

# C arguments
c_args = [
    '-DGETTEXT_PACKAGE="' + meson.project_name() + '"',
    '-DLOCALEDIR="' + get_option('prefix') / get_option('localedir') + '"',
]

# Blueprint compiler
blueprint_compiler = find_program('blueprint-compiler', required: true)

# Symbolic icons list (needed early for resources)
symbolic_icons = [
    'io.github.tobagin.karere-notification-symbolic.svg',
    'io.github.tobagin.karere-do-not-disturb-symbolic.svg',
]

# Generate preferences.blp from template
preferences_blp = configure_file(
    input: 'data/ui/preferences.blp.in',
    output: 'preferences.blp',
    configuration: conf_data
)

# Compile blueprints
blueprints = []

# Compile window.blp
window_ui = custom_target(
    'window.ui',
    input: files('data/ui/window.blp'),
    output: 'window.ui',
    command: [
        blueprint_compiler,
        'compile',
        '--output', '@OUTPUT@',
        '--typelib-path', '/usr/lib/x86_64-linux-gnu/girepository-1.0',
        '@INPUT@'
    ]
)
blueprints += window_ui

# Compile generated preferences.blp
preferences_ui = custom_target(
    'preferences.ui',
    input: preferences_blp,
    output: 'preferences.ui',
    command: [
        blueprint_compiler,
        'compile',
        '--output', '@OUTPUT@',
        '--typelib-path', '/usr/lib/x86_64-linux-gnu/girepository-1.0',
        '@INPUT@'
    ]
)
blueprints += preferences_ui

# Compile shortcuts-dialog.blp
shortcuts_dialog_ui = custom_target(
    'shortcuts-dialog.ui',
    input: files('data/ui/shortcuts-dialog.blp'),
    output: 'shortcuts-dialog.ui',
    command: [
        blueprint_compiler,
        'compile',
        '--output', '@OUTPUT@',
        '--typelib-path', '/usr/lib/x86_64-linux-gnu/girepository-1.0',
        '@INPUT@'
    ]
)
blueprints += shortcuts_dialog_ui


# Copy icons with app-specific names for resources
icon_copies = []
foreach icon : symbolic_icons
    base_name = icon.replace('io.github.tobagin.karere-', '').replace('-symbolic.svg', '')
    icon_copy = custom_target(
        'icon_' + base_name,
        input: 'data/icons/hicolor/symbolic/apps/' + icon,
        output: app_id + '-' + base_name + '-symbolic.svg',
        command: ['cp', '@INPUT@', '@OUTPUT@']
    )
    icon_copies += icon_copy
endforeach

# Generate resources.xml from template
resources_xml = configure_file(
    input: 'data/resources.xml.in',
    output: 'resources.xml',
    configuration: conf_data
)

# Generate resources
resources = gnome.compile_resources(
    'resources',
    resources_xml,
    source_dir: [meson.current_build_dir(), 'data'],
    dependencies: blueprints + icon_copies,
    c_name: 'karere'
)

# Source files
sources = files(
    'src/Main.vala',
    'src/Application.vala',
    'src/Window.vala',
    'src/dialogs/PreferencesDialog.vala',
    'src/dialogs/AboutDialog.vala',
    'src/dialogs/ShortcutsDialog.vala',
    'src/managers/WebKitManager.vala',
    'src/managers/NotificationManager.vala',
    'src/managers/AccessibilityManager.vala',
    'src/managers/KeyboardShortcuts.vala',
    'src/managers/SettingsManager.vala',
    'src/managers/WhatsAppIntegration.vala',
    'src/utils/Utils.vala',
    'src/utils/Constants.vala',
    'src/utils/DependencyContainer.vala',
) + [config_vala]

# Build executable
executable(
    meson.project_name(),
    sources,
    resources,
    dependencies: [
        gtk_dep,
        adwaita_dep,
        webkit_dep,
        gio_dep,
        glib_dep,
        math_dep,
    ],
    vala_args: vala_args,
    c_args: c_args,
    install: true,
    install_dir: get_option('bindir')
)

# Configure development script (commented out for now)
# dev_conf_data = configuration_data()
# dev_conf_data.merge_from(conf_data)
# dev_conf_data.set('BUILD_DIR', meson.current_build_dir())
# dev_conf_data.set('PROJECT_NAME', meson.project_name())

# karere_in = configure_file(
#     input: 'karere.in',
#     output: 'karere-dev',
#     configuration: dev_conf_data,
#     install: false
# )

# Desktop file
desktop_configured = configure_file(
    input: 'data/io.github.tobagin.karere.desktop.in',
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

# Validation tests
desktop_file_validate = find_program('desktop-file-validate', required: false)
if desktop_file_validate.found()
    test('Validate desktop file',
        desktop_file_validate,
        args: [desktop_file],
        suite: 'validation'
    )
endif

# Metainfo file (skip i18n for development builds to avoid ITS rules issues)
if get_option('profile') == 'development'
    metainfo_file = configure_file(
        input: 'data/io.github.tobagin.karere.metainfo.xml.in',
        output: app_id + '.metainfo.xml',
        configuration: conf_data,
        install: true,
        install_dir: get_option('datadir') / 'metainfo'
    )
else
    metainfo_configured = configure_file(
        input: 'data/io.github.tobagin.karere.metainfo.xml.in',
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

# Metainfo validation
appstreamcli = find_program('appstreamcli', required: false)
if appstreamcli.found()
    test('Validate metainfo',
        appstreamcli,
        args: ['validate', '--no-net', metainfo_file],
        suite: 'validation'
    )
endif

# GSchema
gschema_file = configure_file(
    input: 'data/io.github.tobagin.karere.gschema.xml.in',
    output: app_id + '.gschema.xml',
    configuration: conf_data
)

install_data(
    gschema_file,
    install_dir: get_option('datadir') / 'glib-2.0' / 'schemas'
)

# Icons
icon_sizes = ['16', '32', '48', '64', '128', '256', '512']
foreach size : icon_sizes
    install_data(
        'data/icons/hicolor/' + size + 'x' + size + '/apps/io.github.tobagin.karere.png',
        install_dir: get_option('datadir') / 'icons' / 'hicolor' / size + 'x' + size + '/apps',
        rename: app_id + '.png'
    )
endforeach


foreach icon : symbolic_icons
    install_data(
        'data/icons/hicolor/symbolic/apps/' + icon,
        install_dir: get_option('datadir') / 'icons' / 'hicolor' / 'symbolic' / 'apps',
        rename: icon.replace('io.github.tobagin.karere', app_id)
    )
endforeach

# Build script
install_data(
    'scripts/build.sh',
    install_mode: 'rwxr-xr-x',
    install_dir: get_option('bindir')
)

# Subdirectories
subdir('po')

# Tests
if get_option('tests')
    test_env = environment()
    test_env.set('G_TEST_SRCDIR', meson.current_source_dir())
    test_env.set('G_TEST_BUILDDIR', meson.current_build_dir())
    
    # Python test for build configuration
    python3 = find_program('python3', required: false)
    if python3.found()
        test(
            'build-config',
            python3,
            args: [meson.current_source_dir() / 'tests/test_build_config.py'],
            env: test_env,
            workdir: meson.current_source_dir()
        )
    endif
    
    # Vala unit tests
    test_sources = files(
        'tests/test_application.vala',
        'tests/test_window.vala',
        'tests/test_notification_manager.vala',
        'tests/test_runner.vala',
    )
    # Note: test_crash_reporter.vala and test_logger.vala removed (obsolete)
    
    # Sources for tests (excluding Main.vala to avoid conflicts)
    test_lib_sources = files(
        'src/Application.vala',
        'src/Window.vala',
        'src/dialogs/PreferencesDialog.vala',
        'src/dialogs/AboutDialog.vala',
        'src/dialogs/ShortcutsDialog.vala',
        'src/managers/WebKitManager.vala',
        'src/managers/NotificationManager.vala',
        'src/managers/AccessibilityManager.vala',
        'src/managers/KeyboardShortcuts.vala',
        'src/managers/SettingsManager.vala',
        'src/managers/WhatsAppIntegration.vala',
        'src/utils/Utils.vala',
        'src/utils/Constants.vala',
        'src/utils/DependencyContainer.vala',
    ) + [config_vala]
    
    test_executable = executable(
        'test-runner',
        test_sources,
        test_lib_sources,
        resources,
        dependencies: [
            gtk_dep,
            adwaita_dep,
            webkit_dep,
            gio_dep,
            glib_dep,
            math_dep,
        ],
        vala_args: vala_args,
        c_args: c_args,
        install: false
    )
    
    test(
        'unit-tests',
        test_executable,
        env: test_env
    )
endif

# Print summary
summary({
    'Version': meson.project_version(),
    'App ID': app_id,
    'Prefix': get_option('prefix'),
    'Bindir': get_option('bindir'),
    'Datadir': get_option('datadir'),
    'Tests': get_option('tests'),
}, section: 'Configuration')

summary({
    'GTK4': gtk_dep.version(),
    'LibAdwaita': adwaita_dep.version(),
    'WebKitGTK': webkit_dep.version(),
    'GLib': glib_dep.version(),
}, section: 'Dependencies')

# Post-install tasks
gnome.post_install(
    glib_compile_schemas: true,
    gtk_update_icon_cache: true,
    update_desktop_database: true,
)
project(
    'karere',
    'vala', 'c',
    version: '0.1.0',
    meson_version: '>= 1.0.0',
    default_options: [
        'warning_level=2',
        'werror=false',
    ],
    license: 'GPL-3.0-or-later'
)

app_id = 'io.github.tobagin.karere'
app_name = 'Karere'

# Import modules
gnome = import('gnome')
i18n = import('i18n')

# Dependencies
gtk_dep = dependency('gtk4', version: '>= 4.14.0')
adwaita_dep = dependency('libadwaita-1', version: '>= 1.5.0')
webkit_dep = dependency('webkitgtk-6.0', version: '>= 2.40.0')
soup_dep = dependency('libsoup-3.0', version: '>= 3.0.0')
gio_dep = dependency('gio-2.0', version: '>= 2.74.0')
glib_dep = dependency('glib-2.0', version: '>= 2.74.0')
json_glib_dep = dependency('json-glib-1.0', version: '>= 1.6.0')

# Build configuration
cc = meson.get_compiler('c')
valac = meson.get_compiler('vala')

# Configuration data
conf_data = configuration_data()
conf_data.set_quoted('APP_ID', app_id)
conf_data.set_quoted('APP_NAME', app_name)
conf_data.set_quoted('VERSION', meson.project_version())
conf_data.set_quoted('GETTEXT_PACKAGE', meson.project_name())
conf_data.set_quoted('LOCALEDIR', get_option('prefix') / get_option('localedir'))

# Use static config file for now
# config_vala = files('src/config.vala')

# Vala arguments
vala_args = [
    '--target-glib=2.78',
]

# C arguments
c_args = [
    '-DGETTEXT_PACKAGE="' + meson.project_name() + '"',
    '-DLOCALEDIR="' + get_option('prefix') / get_option('localedir') + '"',
]

# Blueprint compiler
blueprint_compiler = find_program('blueprint-compiler', required: true)

# Compile blueprints
blueprints = []
blueprint_files = {
    'window.blp': 'window.ui',
    'preferences.blp': 'preferences.ui',
}

foreach blueprint_file, ui_file : blueprint_files
    blueprint_target = custom_target(
        ui_file,
        input: files('data/ui' / blueprint_file),
        output: ui_file,
        command: [
            blueprint_compiler,
            'compile',
            '--output', '@OUTPUT@',
            '@INPUT@'
        ]
    )
    blueprints += blueprint_target
endforeach

# Generate resources
resources = gnome.compile_resources(
    'resources',
    'data/resources.xml',
    dependencies: blueprints,
    c_name: 'karere'
)

# Source files
sources = files(
    'src/main.vala',
    'src/application.vala',
    'src/window.vala',
    'src/preferences.vala',
    'src/webkit-manager.vala',
    'src/notification-manager.vala',
    'src/logger.vala',
    'src/crash-reporter.vala',
    'src/utils.vala',
    'src/config.vala',
)

# Build executable
executable(
    meson.project_name(),
    sources,
    resources,
    dependencies: [
        gtk_dep,
        adwaita_dep,
        webkit_dep,
        soup_dep,
        gio_dep,
        glib_dep,
        json_glib_dep,
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
desktop_file = i18n.merge_file(
    input: 'data' / app_id + '.desktop.in',
    output: app_id + '.desktop',
    type: 'desktop',
    po_dir: 'po',
    install: true,
    install_dir: get_option('datadir') / 'applications'
)

# AppData file
appdata_file = i18n.merge_file(
    input: 'data' / app_id + '.appdata.xml.in',
    output: app_id + '.appdata.xml',
    po_dir: 'po',
    install: true,
    install_dir: get_option('datadir') / 'metainfo'
)

# GSchema
install_data(
    'data' / app_id + '.gschema.xml',
    install_dir: get_option('datadir') / 'glib-2.0' / 'schemas'
)

# Icons
icon_sizes = ['16', '32', '48', '64', '128', '256', '512']
foreach size : icon_sizes
    install_data(
        'data/icons/hicolor/' + size + 'x' + size + '/apps/' + app_id + '.png',
        install_dir: get_option('datadir') / 'icons' / 'hicolor' / size + 'x' + size + '/apps'
    )
endforeach

# install_data(
#     'data/icons/hicolor/scalable/apps/' + app_id + '.svg',
#     install_dir: get_option('datadir') / 'icons' / 'hicolor' / 'scalable' / 'apps'
# )

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
        'tests/test_runner.vala',
    )
    
    # Sources for tests (excluding main.vala to avoid conflicts)
    test_lib_sources = files(
        'src/application.vala',
        'src/window.vala',
        'src/preferences.vala',
        'src/webkit-manager.vala',
        'src/notification-manager.vala',
        'src/logger.vala',
        'src/crash-reporter.vala',
        'src/utils.vala',
        'src/config.vala',
    )
    
    test_executable = executable(
        'test-runner',
        test_sources,
        test_lib_sources,
        resources,
        dependencies: [
            gtk_dep,
            adwaita_dep,
            webkit_dep,
            soup_dep,
            gio_dep,
            glib_dep,
            json_glib_dep,
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
    'LibSoup': soup_dep.version(),
    'GLib': glib_dep.version(),
}, section: 'Dependencies')
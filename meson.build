project(
    'karere',
    'vala', 'c',
    version: '0.1.0',
    meson_version: '>= 1.8.0',
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
gtk_dep = dependency('gtk4', version: '>= 4.19.0')
adwaita_dep = dependency('libadwaita-1', version: '>= 1.7.0')
webkit_dep = dependency('webkit2gtk-6.0', version: '>= 2.44.0')
soup_dep = dependency('libsoup-3.0', version: '>= 3.4.0')
gio_dep = dependency('gio-2.0', version: '>= 2.78.0')
glib_dep = dependency('glib-2.0', version: '>= 2.78.0')
json_glib_dep = dependency('json-glib-1.0', version: '>= 1.8.0')

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

# Generate config.vala
config_vala = configure_file(
    input: 'src/config.vala.in',
    output: 'config.vala',
    configuration: conf_data
)

# Vala arguments
vala_args = [
    '--target-glib=2.78',
    '--gresources', 'data/resources.xml',
]

# C arguments
c_args = [
    '-DGETTEXT_PACKAGE="' + meson.project_name() + '"',
    '-DLOCALEDIR="' + get_option('prefix') / get_option('localedir') + '"',
]

# Blueprint compiler
blueprint_compiler = find_program('blueprint-compiler', required: true)

# Compile blueprints
blueprints_dir = 'data/ui'
blueprints = custom_target(
    'blueprints',
    input: files(
        blueprints_dir / 'window.blp',
        blueprints_dir / 'preferences.blp',
    ),
    output: [
        'window.ui',
        'preferences.ui',
    ],
    command: [
        blueprint_compiler,
        'batch-compile',
        '@OUTPUT_DIR@',
        '@CURRENT_SOURCE_DIR@' / blueprints_dir,
        '@INPUT@'
    ],
    depend_files: files(
        blueprints_dir / 'window.blp',
        blueprints_dir / 'preferences.blp',
    )
)

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
)

# Build executable
executable(
    meson.project_name(),
    sources,
    config_vala,
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

# Install helper script
karere_in = configure_file(
    input: 'karere.in',
    output: 'karere-dev',
    configuration: conf_data,
    install: false
)

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

install_data(
    'data/icons/hicolor/scalable/apps/' + app_id + '.svg',
    install_dir: get_option('datadir') / 'icons' / 'hicolor' / 'scalable' / 'apps'
)

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

# Data Configuration
desktop_conf = configuration_data()
desktop_conf.set('APP_ID', base_id)
desktop_conf.set('APP_NAME', app_name)

# Desktop File
i18n.merge_file(
  input: configure_file(
    input: 'io.github.tobagin.karere.desktop.in',
    output: 'io.github.tobagin.karere.desktop.in',
    configuration: desktop_conf
  ),
  output: base_id + '.desktop',
  type: 'desktop',
  po_dir: '../po',
  install: true,
  install_dir: get_option('datadir') / 'applications'
)

# Metainfo File
i18n.merge_file(
  input: configure_file(
    input: 'io.github.tobagin.karere.metainfo.xml.in',
    output: 'io.github.tobagin.karere.metainfo.xml.in',
    configuration: desktop_conf
  ),
  output: base_id + '.metainfo.xml',
  po_dir: '../po',
  install: true,
  install_dir: get_option('datadir') / 'metainfo'
)

# GSchema
# Configure schema with App ID
schema_conf = configuration_data()
schema_conf.set('APP_ID', base_id)
# Ensure path ends with / and format is typically /tld/vendor/app/
# base_id is like io.github.tobagin.karere(.Devel)
# We want /io/github/tobagin/karere/ or /io/github/tobagin/karere/Devel/
schema_path = '/' + base_id.replace('.', '/') + '/'
schema_conf.set('APP_PATH', schema_path)

configure_file(
  input: 'io.github.tobagin.karere.gschema.xml.in',
  output: base_id + '.gschema.xml',
  configuration: schema_conf,
  install: true,
  install_dir: get_option('datadir') / 'glib-2.0/schemas'
)

gnome.compile_schemas()

# Blueprint Compilation
blueprint_compiler = find_program('blueprint-compiler', required: true)

blueprint_files = ['window', 'preferences', 'keyboard-shortcuts']
blueprint_targets = []

foreach name : blueprint_files
  blueprint_targets += custom_target(
    name + '_ui',
    input: 'ui/' + name + '.blp',
    output: name + '.ui',
    command: [blueprint_compiler, 'compile', '--output', '@OUTPUT@', '@INPUT@'],
  )
endforeach


# Resources
gnome.compile_resources(
  'karere',
  'resources.gresource.xml',
  gresource_bundle: true,
  install: true,
  install_dir: get_option('datadir') / 'karere',
  dependencies: blueprint_targets
)


# Icons
if get_option('profile') == 'development'
  icon_src_id = 'io.github.tobagin.karere.Devel'
else
  icon_src_id = 'io.github.tobagin.karere'
endif

# Install Scalable Icon
install_data(
  'icons/hicolor/scalable/apps/' + icon_src_id + '.svg',
  rename: base_id + '.svg',
  install_dir: get_option('datadir') / 'icons/hicolor/scalable/apps'
)

# Install PNG Icons at various sizes
foreach size : ['48x48', '64x64', '128x128', '256x256', '512x512']
  install_data(
    'icons/hicolor/' + size + '/apps/' + icon_src_id + '.png',
    rename: base_id + '.png',
    install_dir: get_option('datadir') / 'icons/hicolor' / size / 'apps'
  )
endforeach


# Install Symbolic Icon
install_data(
  'icons/hicolor/symbolic/apps/' + icon_src_id + '-symbolic.svg',
  rename: base_id + '-symbolic.svg',
  install_dir: get_option('datadir') / 'icons/hicolor/symbolic/apps'
)

# Install Extra Symbolics
extra_symbolics = ['new-message', 'notification', 'do-not-disturb']
foreach s : extra_symbolics
  install_data(
    'icons/hicolor/symbolic/apps/' + icon_src_id + '-' + s + '-symbolic.svg',
    rename: base_id + '-' + s + '-symbolic.svg',
    install_dir: get_option('datadir') / 'icons/hicolor/symbolic/apps'
  )
endforeach


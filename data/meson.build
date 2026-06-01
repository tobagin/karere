# Desktop entry
desktop_conf = configuration_data()
desktop_conf.set('APP_ID', base_id)
desktop_conf.set('APP_NAME', app_name)

i18n.merge_file(
  input: configure_file(
    input: 'io.github.tobagin.karere.desktop.in',
    output: 'io.github.tobagin.karere.desktop.in',
    configuration: desktop_conf,
  ),
  output: base_id + '.desktop',
  type: 'desktop',
  po_dir: meson.project_source_root() / 'po',
  install: true,
  install_dir: get_option('datadir') / 'applications',
)

# AppStream metainfo
i18n.merge_file(
  input: configure_file(
    input: 'io.github.tobagin.karere.metainfo.xml.in',
    output: 'io.github.tobagin.karere.metainfo.xml.in',
    configuration: desktop_conf,
  ),
  output: base_id + '.metainfo.xml',
  po_dir: meson.project_source_root() / 'po',
  install: true,
  install_dir: get_option('datadir') / 'metainfo',
)

# GSchema — substitute @APP_ID@/@APP_PATH@ then install for glib-compile-schemas.
app_path = '/' + '/'.join(base_id.split('.')) + '/'
schema_conf = configuration_data()
schema_conf.set('APP_ID', base_id)
schema_conf.set('APP_PATH', app_path)
configure_file(
  input: 'io.github.tobagin.karere.gschema.xml.in',
  output: base_id + '.gschema.xml',
  configuration: schema_conf,
  install: true,
  install_dir: get_option('datadir') / 'glib-2.0' / 'schemas',
)

# Blueprint -> .ui pre-compilation. build.rs (cargo) also compiles these into
# $OUT_DIR/ui/ for the embedded gresource; here we materialise them in the
# source tree so the meson POT target (which reads POTFILES.in entries like
# data/ui/window.ui) can extract translatable strings via xgettext.
blueprint_compiler = find_program('blueprint-compiler', required: true)
blueprint_files = ['window', 'preferences', 'keyboard-shortcuts']
foreach name : blueprint_files
  run_command(
    blueprint_compiler,
    'compile',
    '--output', meson.project_source_root() / 'data' / 'ui' / (name + '.ui'),
    meson.project_source_root() / 'data' / 'ui' / (name + '.blp'),
    check: true,
  )
endforeach

# gresource bundle (also embedded into the binary at build time via build.rs;
# installed here so external tooling and karere v3-compatible install layout
# work too).
install_data(
  'karere.gresource.xml',
  install_dir: get_option('datadir') / 'karere',
)

# Notification sounds
sounds_dir = get_option('datadir') / 'karere' / 'sounds'
install_data(
  'sounds/alert.oga',
  'sounds/pop.oga',
  'sounds/soft.oga',
  'sounds/start.oga',
  'sounds/whatsapp.oga',
  install_dir: sounds_dir,
)

# Icons
if get_option('profile') == 'development'
  icon_src_id = 'io.github.tobagin.karere.Devel'
else
  icon_src_id = 'io.github.tobagin.karere'
endif

install_data(
  'icons/hicolor/scalable/apps/' + icon_src_id + '.svg',
  rename: base_id + '.svg',
  install_dir: get_option('datadir') / 'icons/hicolor/scalable/apps',
)

foreach size : ['48x48', '64x64', '128x128', '256x256', '512x512']
  install_data(
    'icons/hicolor/' + size + '/apps/' + icon_src_id + '.png',
    rename: base_id + '.png',
    install_dir: get_option('datadir') / 'icons/hicolor' / size / 'apps',
  )
endforeach

install_data(
  'icons/hicolor/symbolic/apps/' + icon_src_id + '-symbolic.svg',
  rename: base_id + '-symbolic.svg',
  install_dir: get_option('datadir') / 'icons/hicolor/symbolic/apps',
)

# M15 adds `tray` / `tray-unread`. App-id-prefixed so Flatpak exports them to
# the host icon theme (it only exports `$FLATPAK_ID*`); the SNI host resolves
# the name there. `tray.rs` builds the name from FLATPAK_ID / APP_ID at runtime.
extra_symbolics = ['new-message', 'notification', 'do-not-disturb', 'tray', 'tray-unread']
foreach s : extra_symbolics
  install_data(
    'icons/hicolor/symbolic/apps/' + icon_src_id + '-' + s + '-symbolic.svg',
    rename: base_id + '-' + s + '-symbolic.svg',
    install_dir: get_option('datadir') / 'icons/hicolor/symbolic/apps',
  )
endforeach

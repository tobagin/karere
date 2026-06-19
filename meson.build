project('karere', 'rust',
          version: '4.0.5',
    meson_version: '>= 0.59.0',
  default_options: [ 'warning_level=2', 'werror=false', ],
)

i18n = import('i18n')
gnome = import('gnome')

profile = get_option('profile')
if profile == 'development'
  base_id = 'io.github.tobagin.karere.Devel'
  app_name = 'Karere (Devel)'
else
  base_id = 'io.github.tobagin.karere'
  app_name = 'Karere'
endif

dependency('glib-2.0', version: '>= 2.66')
dependency('gio-2.0', version: '>= 2.66')
dependency('gtk4', version: '>= 4.12')
dependency('libadwaita-1', version: '>= 1.6')

cargo = find_program('cargo', required: true)
fs = import('fs')

output = 'karere'

cargo_options = [ '--manifest-path', meson.project_source_root() / 'Cargo.toml' ]
cargo_options += [ '--target-dir', meson.project_build_root() / 'target' ]
cargo_options += [ '--release' ]
rust_target = 'release'

if fs.is_dir(meson.project_source_root() / 'cargo')
  cargo_env = [ 'CARGO_HOME=' + meson.project_source_root() / 'cargo' ]
else
  cargo_env = [ 'CARGO_HOME=' + meson.project_build_root() / 'cargo-home' ]
endif

# Thread the profile/app-id into the Rust build so the binary registers the
# matching app-id (.Devel) + GSettings schema and shows the devel headerbar.
cargo_env += [ 'KARERE_APP_ID=' + base_id ]
cargo_env += [ 'KARERE_PROFILE=' + profile ]

custom_target('cargo-build',
  build_by_default: true,
  build_always_stale: true,
  output: output,
  console: true,
  install: true,
  install_dir: get_option('bindir'),
  command: [
    'sh', '-c',
    'env @0@ @1@ build @2@ && cp @3@ @OUTPUT@'.format(
      ' '.join(cargo_env),
      cargo.full_path(),
      ' '.join(cargo_options),
      meson.project_build_root() / 'target' / rust_target / 'karere',
    ),
  ],
)

subdir('data')
subdir('po')  # po/meson.build: i18n.gettext('karere', preset: 'glib'), verbatim from v3

gnome.post_install(
  glib_compile_schemas: true,
  update_desktop_database: true,
  gtk_update_icon_cache: true,
)

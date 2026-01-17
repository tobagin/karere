project('karere', 'rust',
          version: '2.3.0',
    meson_version: '>= 0.59.0',
  default_options: [ 'warning_level=2', 'werror=false', ],
)

i18n = import('i18n')
gnome = import('gnome')


profile = get_option('profile')
if profile == 'development'
  base_id = 'io.github.tobagin.karere.Dev'
  app_name = 'Karere (Dev)'
else
  base_id = 'io.github.tobagin.karere'
  app_name = 'Karere'
endif


dependency('glib-2.0', version: '>= 2.66')
dependency('gio-2.0', version: '>= 2.66')
dependency('gtk4', version: '>= 4.0.0')
dependency('libadwaita-1', version: '>= 1.0.0')

cargo = find_program('cargo', required: true)
fs = import('fs')

output = meson.project_name()

# Cargo build logic adapted from Ntfyr
cargo_options = [ '--manifest-path', meson.project_source_root() / 'Cargo.toml' ]
cargo_options += [ '--target-dir', meson.project_build_root() / 'target' ]

if get_option('profile') == 'development'
  # cargo_options += [ '--offline' ]
  # Development profile should have network access (via --share=network in manifest) so we don't need offline mode
  # In dev profile, we might still want release-like optimizations or debug:
  # Ntfyr used 'debug' logic here, but Karere used 'release'. Sticking to release for now as per previous setup.
  cargo_options += [ '--release' ]
  rust_target = 'release'
else
  cargo_options += [ '--release', '--offline' ]
  rust_target = 'release'
endif

if fs.is_dir(meson.project_source_root() / 'cargo')
  cargo_env = [ 'CARGO_HOME=' + meson.project_source_root() / 'cargo' ]
else
  cargo_env = [ 'CARGO_HOME=' + meson.project_build_root() / 'cargo-home' ]
endif

custom_target('cargo-build',
  build_by_default: true,
  build_always_stale: true,
  output: output,
  console: true,
  install: true,
  install_dir: get_option('bindir'),
  command: [
    'env',
    cargo_env,
    cargo, 'build',
    cargo_options,
    '&&',
    'cp', meson.project_build_root() / 'target' / rust_target / 'karere', '@OUTPUT@',
  ],
)

subdir('data')
subdir('po')

gnome.post_install(
  glib_compile_schemas: true,
  update_desktop_database: true,
  gtk_update_icon_cache: true,
)

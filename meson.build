project('karere', 'rust',
          version: '2.0.8',
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

cargo_script = find_program('build-aux/cargo.sh')

output = 'karere'

# Detect vendored cargo sources (Flatpak)
if fs.is_dir('cargo') and profile != 'development'
  message('Found vendored cargo sources, enabling offline build')
  cargo_options = [ '--release', '--offline' ]
  cargo_home = meson.project_source_root() / 'cargo'
else
  message('No vendored cargo sources found, using default build')
  cargo_options = [ '--release' ]
  cargo_home = meson.project_build_root() / 'cargo-home'
endif

message('Building with Cargo options: @0@'.format(cargo_options))

cargo_env = {
  'CARGO_HOME': cargo_home,
  'CARGO_TARGET_DIR': meson.project_build_root() / 'target',
}

custom_target('cargo-build',
  build_by_default: true,
  build_always_stale: true,
  output: output,
  console: true,
  install: true,
  install_dir: get_option('bindir'),
  command: [
    cargo_script,
    meson.project_build_root(),
    meson.project_source_root(),
    '@OUTPUT@',
    cargo_options,
  ],
  env: cargo_env,
)

subdir('data')
subdir('po')

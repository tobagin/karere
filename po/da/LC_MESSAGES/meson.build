# Generated for da — do not edit manually (hand-rolled i18n wiring)
gmotarget = custom_target('karere-da.mo',
  input: meson.project_source_root() / 'po' / 'da.po',
  output: 'karere.mo',
  command: [msgfmt, '-o', '@OUTPUT@', '@INPUT@'],
  install: true,
  install_dir: get_option('localedir') / 'da' / 'LC_MESSAGES',
  install_tag: 'i18n',
)
gmotargets += gmotarget

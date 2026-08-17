# Generated for de_DE — do not edit manually (hand-rolled i18n wiring)
gmotarget = custom_target('karere-de_DE.mo',
  input: meson.project_source_root() / 'po' / 'de_DE.po',
  output: 'karere.mo',
  command: [msgfmt, '-o', '@OUTPUT@', '@INPUT@'],
  install: true,
  install_dir: get_option('localedir') / 'de_DE' / 'LC_MESSAGES',
  install_tag: 'i18n',
)
gmotargets += gmotarget

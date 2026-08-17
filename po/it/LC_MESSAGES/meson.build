# Generated for it — do not edit manually (hand-rolled i18n wiring)
gmotarget = custom_target('karere-it.mo',
  input: meson.project_source_root() / 'po' / 'it.po',
  output: 'karere.mo',
  command: [msgfmt, '-o', '@OUTPUT@', '@INPUT@'],
  install: true,
  install_dir: get_option('localedir') / 'it' / 'LC_MESSAGES',
  install_tag: 'i18n',
)
gmotargets += gmotarget

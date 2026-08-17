# Generated for bn — do not edit manually (hand-rolled i18n wiring)
gmotarget = custom_target('karere-bn.mo',
  input: meson.project_source_root() / 'po' / 'bn.po',
  output: 'karere.mo',
  command: [msgfmt, '-o', '@OUTPUT@', '@INPUT@'],
  install: true,
  install_dir: get_option('localedir') / 'bn' / 'LC_MESSAGES',
  install_tag: 'i18n',
)
gmotargets += gmotarget

# Generated for es_MX — do not edit manually (hand-rolled i18n wiring)
gmotarget = custom_target('karere-es_MX.mo',
  input: meson.project_source_root() / 'po' / 'es_MX.po',
  output: 'karere.mo',
  command: [msgfmt, '-o', '@OUTPUT@', '@INPUT@'],
  install: true,
  install_dir: get_option('localedir') / 'es_MX' / 'LC_MESSAGES',
  install_tag: 'i18n',
)
gmotargets += gmotarget

# Generated for es_AR — do not edit manually (hand-rolled i18n wiring)
gmotarget = custom_target('karere-es_AR.mo',
  input: meson.project_source_root() / 'po' / 'es_AR.po',
  output: 'karere.mo',
  command: [msgfmt, '-o', '@OUTPUT@', '@INPUT@'],
  install: true,
  install_dir: get_option('localedir') / 'es_AR' / 'LC_MESSAGES',
  install_tag: 'i18n',
)
gmotargets += gmotarget

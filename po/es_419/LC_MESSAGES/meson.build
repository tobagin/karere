# Generated for es_419 — do not edit manually (hand-rolled i18n wiring)
gmotarget = custom_target('karere-es_419.mo',
  input: meson.project_source_root() / 'po' / 'es_419.po',
  output: 'karere.mo',
  command: [msgfmt, '-o', '@OUTPUT@', '@INPUT@'],
  install: true,
  install_dir: get_option('localedir') / 'es_419' / 'LC_MESSAGES',
  install_tag: 'i18n',
)
gmotargets += gmotarget

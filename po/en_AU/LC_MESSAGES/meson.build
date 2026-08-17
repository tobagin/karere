# Generated for en_AU — do not edit manually (hand-rolled i18n wiring)
gmotarget = custom_target('karere-en_AU.mo',
  input: meson.project_source_root() / 'po' / 'en_AU.po',
  output: 'karere.mo',
  command: [msgfmt, '-o', '@OUTPUT@', '@INPUT@'],
  install: true,
  install_dir: get_option('localedir') / 'en_AU' / 'LC_MESSAGES',
  install_tag: 'i18n',
)
gmotargets += gmotarget

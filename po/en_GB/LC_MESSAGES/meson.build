# Generated for en_GB — do not edit manually (hand-rolled i18n wiring)
gmotarget = custom_target('karere-en_GB.mo',
  input: meson.project_source_root() / 'po' / 'en_GB.po',
  output: 'karere.mo',
  command: [msgfmt, '-o', '@OUTPUT@', '@INPUT@'],
  install: true,
  install_dir: get_option('localedir') / 'en_GB' / 'LC_MESSAGES',
  install_tag: 'i18n',
)
gmotargets += gmotarget

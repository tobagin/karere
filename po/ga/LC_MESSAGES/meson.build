# Generated for ga — do not edit manually (hand-rolled i18n wiring)
gmotarget = custom_target('karere-ga.mo',
  input: meson.project_source_root() / 'po' / 'ga.po',
  output: 'karere.mo',
  command: [msgfmt, '-o', '@OUTPUT@', '@INPUT@'],
  install: true,
  install_dir: get_option('localedir') / 'ga' / 'LC_MESSAGES',
  install_tag: 'i18n',
)
gmotargets += gmotarget

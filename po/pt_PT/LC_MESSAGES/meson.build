# Generated for pt_PT — do not edit manually (hand-rolled i18n wiring)
gmotarget = custom_target('karere-pt_PT.mo',
  input: meson.project_source_root() / 'po' / 'pt_PT.po',
  output: 'karere.mo',
  command: [msgfmt, '-o', '@OUTPUT@', '@INPUT@'],
  install: true,
  install_dir: get_option('localedir') / 'pt_PT' / 'LC_MESSAGES',
  install_tag: 'i18n',
)
gmotargets += gmotarget

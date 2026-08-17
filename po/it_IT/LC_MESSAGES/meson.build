# Generated for it_IT — do not edit manually (hand-rolled i18n wiring)
gmotarget = custom_target('karere-it_IT.mo',
  input: meson.project_source_root() / 'po' / 'it_IT.po',
  output: 'karere.mo',
  command: [msgfmt, '-o', '@OUTPUT@', '@INPUT@'],
  install: true,
  install_dir: get_option('localedir') / 'it_IT' / 'LC_MESSAGES',
  install_tag: 'i18n',
)
gmotargets += gmotarget

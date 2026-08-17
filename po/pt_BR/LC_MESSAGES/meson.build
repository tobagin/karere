# Generated for pt_BR — do not edit manually (hand-rolled i18n wiring)
gmotarget = custom_target('karere-pt_BR.mo',
  input: meson.project_source_root() / 'po' / 'pt_BR.po',
  output: 'karere.mo',
  command: [msgfmt, '-o', '@OUTPUT@', '@INPUT@'],
  install: true,
  install_dir: get_option('localedir') / 'pt_BR' / 'LC_MESSAGES',
  install_tag: 'i18n',
)
gmotargets += gmotarget

# Generated for ro — do not edit manually (hand-rolled i18n wiring)
gmotarget = custom_target('karere-ro.mo',
  input: meson.project_source_root() / 'po' / 'ro.po',
  output: 'karere.mo',
  command: [msgfmt, '-o', '@OUTPUT@', '@INPUT@'],
  install: true,
  install_dir: get_option('localedir') / 'ro' / 'LC_MESSAGES',
  install_tag: 'i18n',
)
gmotargets += gmotarget

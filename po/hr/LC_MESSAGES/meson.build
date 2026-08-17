# Generated for hr — do not edit manually (hand-rolled i18n wiring)
gmotarget = custom_target('karere-hr.mo',
  input: meson.project_source_root() / 'po' / 'hr.po',
  output: 'karere.mo',
  command: [msgfmt, '-o', '@OUTPUT@', '@INPUT@'],
  install: true,
  install_dir: get_option('localedir') / 'hr' / 'LC_MESSAGES',
  install_tag: 'i18n',
)
gmotargets += gmotarget

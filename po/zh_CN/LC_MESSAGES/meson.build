# Generated for zh_CN — do not edit manually (hand-rolled i18n wiring)
gmotarget = custom_target('karere-zh_CN.mo',
  input: meson.project_source_root() / 'po' / 'zh_CN.po',
  output: 'karere.mo',
  command: [msgfmt, '-o', '@OUTPUT@', '@INPUT@'],
  install: true,
  install_dir: get_option('localedir') / 'zh_CN' / 'LC_MESSAGES',
  install_tag: 'i18n',
)
gmotargets += gmotarget

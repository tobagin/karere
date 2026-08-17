# Generated for zh_TW — do not edit manually (hand-rolled i18n wiring)
gmotarget = custom_target('karere-zh_TW.mo',
  input: meson.project_source_root() / 'po' / 'zh_TW.po',
  output: 'karere.mo',
  command: [msgfmt, '-o', '@OUTPUT@', '@INPUT@'],
  install: true,
  install_dir: get_option('localedir') / 'zh_TW' / 'LC_MESSAGES',
  install_tag: 'i18n',
)
gmotargets += gmotarget

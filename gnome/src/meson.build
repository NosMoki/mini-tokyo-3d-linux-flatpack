gnome.compile_resources(app_id + '.src',
  app_id + '.src.gresource.xml',
  gresource_bundle: true,
  install: true,
  install_dir: pkgdatadir,
)

launcher_conf = configuration_data()
launcher_conf.set('GJS', find_program('gjs').full_path())
launcher_conf.set('APP_ID', app_id)
launcher_conf.set('VERSION', meson.project_version())
launcher_conf.set('PREFIX', get_option('prefix'))
launcher_conf.set('LIBDIR', get_option('prefix') / get_option('libdir'))
configure_file(
  input: app_id + '.in',
  output: 'mini-tokyo-3d',
  configuration: launcher_conf,
  install: true,
  install_dir: get_option('bindir'),
  install_mode: 'rwxr-xr-x',
)

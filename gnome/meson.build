gnome = import('gnome')
i18n = import('i18n')
fs = import('fs')

app_id = 'com.minitokyo3d.MiniTokyo3D'
pkgdatadir = get_option('prefix') / get_option('datadir') / app_id
webdir = pkgdatadir / 'web'

# The desktop app shares its version with the web bundle in package.json.
package_version = fs.read(meson.project_source_root() / 'package.json').split('"version": "')[1].split('"')[0]
if package_version != meson.project_version()
  error('The version in meson.build (@0@) does not match package.json (@1@)'.format(meson.project_version(), package_version))
endif

subdir('data')
subdir('src')
subdir('web')
subdir('po')

gnome.post_install(
  glib_compile_schemas: true,
  gtk_update_icon_cache: true,
  update_desktop_database: true,
)

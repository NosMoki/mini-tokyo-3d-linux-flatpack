# The web bundle built by `npm run build`, served to the app's web view from
# the install directory (see src/scheme.js).
dist = meson.project_source_root() / 'dist'
if not fs.is_file(dist / 'mini-tokyo-3d.min.js') or not fs.is_file(dist / 'assets' / 'style.json')
  error('The web bundle was not found in dist/. Run "npm install && npm run build" in @0@ first.'.format(meson.project_source_root()))
endif

install_data(
  dist / 'mini-tokyo-3d.min.js',
  dist / 'mini-tokyo-3d.min.css',
  install_dir: webdir,
)
install_subdir(dist / 'assets', install_dir: webdir)

# Optional plugins, in the same order and with the same factory calls as
# scripts/plugins.mjs uses for the web build.
plugins = [
  ['plugin_precipitation', 'mt3dPrecipitation()'],
  ['plugin_fireworks', 'mt3dFireworks()'],
  ['plugin_livecam', 'mt3dLivecam()'],
  ['plugin_plateau', 'mt3dPlateau({enabled: false})'],
  ['plugin_gtfs', 'mt3dGtfs({enabled: false})'],
]
plugin_scripts = []
plugin_factories = []
foreach plugin : plugins
  path = get_option(plugin[0])
  if path != ''
    if not fs.is_file(path)
      error('-D@0@=@1@ does not point at a file'.format(plugin[0], path))
    endif
    install_data(path, install_dir: webdir)
    plugin_scripts += '<script src="@0@"></script>'.format(fs.name(path))
    plugin_factories += plugin[1]
  endif
endforeach

configure_file(
  input: 'index.html.in',
  output: 'index.html',
  configuration: {
    'PLUGIN_SCRIPTS': '\n\t'.join(plugin_scripts),
    'PLUGINS': ', '.join(plugin_factories),
  },
  install: true,
  install_dir: webdir,
)

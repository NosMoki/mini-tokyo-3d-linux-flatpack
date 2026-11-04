# Mini Tokyo 3D for GNOME

<<<<<<< Updated upstream
A native GNOME app for Fedora (and any Linux with Flatpak) that runs Mini Tokyo
3D in its own window.

The app is a GTK 4 and libadwaita window, written in Rust, around the Mini Tokyo
3D web app, which runs **unchanged** in WebKitGTK. Everything that
minitokyo3d.com does works the same way, because it is the same code:

- the 3D trains, buses and flights, with the website's sizes and scaling as you
  zoom, and the railway and bus routes
- the live cameras, precipitation, fireworks, PLATEAU and GTFS plugins, at the
  versions that the website loads
- train information, station details, search, playback, the underground view,
  eco mode and the map's own fullscreen button

The GNOME parts are a header bar with the main menu, a preferences dialog, a
keyboard shortcuts dialog, an about dialog, fullscreen with F11, the system's
light or dark style, and a window that remembers its size.

## Install

```sh
gnome/build-flatpak.sh            # build, then install for the current user
gnome/build-flatpak.sh --bundle   # also write gnome/_build/mini-tokyo-3d.flatpak
```

The script needs `node`/`npm` (with `npm install` already run in the repository)
and `flatpak`. It installs the GNOME 51 SDK, the Rust SDK extension and
flatpak-builder from Flathub for the current user if they are missing (about
1 GB the first time). Mini Tokyo 3D then appears in the app grid, or run
`flatpak run io.github.nosmoki.MiniTokyo3D`.

To install the bundle on another computer:
`flatpak install --user mini-tokyo-3d.flatpak`.

## Access tokens

On first launch the app asks for a **Mapbox access token**, which the 3D map
needs. Create one for free at <https://account.mapbox.com/access-tokens/> (the
default public token, starting with `pk.`, is fine). The token minitokyo3d.com
uses only works on that website.

In Preferences you can also add **ODPT access tokens** from
<https://developer.odpt.org/>. They are optional: minitokyo3d.com uses them for
the live positions and service updates of some operators (Toei, Keikyu, Tobu
and others). Without them those trains run by their timetables.

Tokens are kept in GNOME Keyring (through the Secret portal in the Flatpak) and
are never written to disk in the clear.

## Command line

```
flatpak run io.github.nosmoki.MiniTokyo3D [OPTION…]

  -l, --lang=LANG          Language of the map, such as en or ja
  --selection=ID           Train, flight or station to select at startup
  --gtfs-url=URL           Show only this GTFS feed (needs --gtfs-color)
  --gtfs-vp-url=URL        GTFS Realtime vehicle positions for --gtfs-url
  --gtfs-color=RRGGBB      Route color for --gtfs-url
  --version                Show the version
```

These are the website's URL parameters (`lang`, `selection`, `gtfsurl`,
`gtfsvpurl`, `gtfscolor`). Running the command again while the app is open
applies them to the open window.

## How it works

`build-web.mjs` stages the web app into `_build/web`:

1. `npm run build` bundles the core library into `dist/`.
2. The five plugins are downloaded from npm at pinned versions and checked
   against their integrity hashes.
3. `scripts/gen-index.mjs` generates `index.html`, as for the website, but with
   placeholders for the tokens and without analytics.

The app serves that directory to its WebView from a small HTTP server on
127.0.0.1 (`src/server.rs`). It uses HTTP rather than a custom URI scheme
because the live cameras are YouTube players, which refuse to play without a
Referer, and WebKit only sends one from http(s) pages. The server answers only
requests for its own address, under a random path, and fills the tokens from
the keyring into `index.html` as it serves it.

Links that leave the map (attributions, the about panel and so on) open in the
web browser. Right-click is left to the map, which uses it to rotate, except in
text fields.

| File | |
| --- | --- |
| `src/main.rs` | Application, command line options, about dialog |
| `src/window.rs` | Main window, WebView, token setup and error pages |
| `src/server.rs` | Local HTTP server for the web app |
| `src/secrets.rs` | Tokens in GNOME Keyring, Mapbox token check |
| `src/preferences.rs` | Preferences dialog |
| `src/shortcuts.rs` | Keyboard shortcuts dialog |
| `src/dev.rs` | Test hooks for debug builds |
| `build-web.mjs` | Stages the web app and plugins |
| `io.github.nosmoki.MiniTokyo3D.yml` | Flatpak manifest |
| `data/` | Desktop entry, AppStream metadata, GSettings schema, icons |

## Development

Building outside Flatpak needs the development packages:

```sh
sudo dnf install gtk4-devel libadwaita-devel webkitgtk6.0-devel libsecret-devel
node gnome/build-web.mjs
cd gnome && cargo run
```

`cargo run` finds the staged web app, the GSettings schema and the icon in the
source tree. Debug builds enable the Web Inspector (right-click, Inspect
Element) and print the page's console messages. `src/dev.rs` lists environment
variables for testing without a person at the screen, such as
`MT3D_DEV_MAPBOX_TOKEN` and `MT3D_DEV_SCREENSHOTS`.

The Flatpak build downloads crates during the build, which is fine for local
builds. Publishing on Flathub would need offline sources generated with
flatpak-builder-tools (`flatpak-cargo-generator`).
=======
A native GNOME desktop app for Fedora Workstation. It runs the unchanged Mini
Tokyo 3D web app, built from `src/` by `npm run build`, in a GTK 4 and
libadwaita window. Every feature, panel and control of the map works the same
as on the web. The window adds the parts a GNOME app is expected to have: a
header bar with a main menu, Preferences, Keyboard Shortcuts and an About
dialog.

## How it works

| Part | Technology |
| --- | --- |
| Window, menus, dialogs | GTK 4 + libadwaita, written in GJS (GNOME's JavaScript) |
| Map | The web bundle in `dist/`, rendered by WebKitGTK 6.0 (WebGL 2) |
| Access tokens | GNOME Keyring, through libsecret |
| Settings (language, window size) | GSettings |
| Build and install | Meson; RPM package for Fedora |

The web bundle is served to the web view from the install directory through a
private `mt3d://` URI scheme. The page therefore gets a secure origin, which
`fetch()`, workers and the Cache API need, without running a local web server.
The access tokens are passed to the page directly and never leave the app.

```
meson.build, meson.options      Meson project (repository root)
gnome/
├── src/                        GJS sources and GtkBuilder UI files
│   ├── main.js                 Application: actions, command line, About dialog
│   ├── window.js / window.ui   Main window hosting the web view
│   ├── preferences.js / .ui    Preferences dialog
│   ├── shortcuts-dialog.ui     Keyboard Shortcuts dialog
│   ├── scheme.js               mt3d:// scheme serving the web bundle
│   └── secrets.js              Keyring access
├── web/index.html.in           Entry page, derived from public/index.html
├── data/                       Desktop entry, AppStream metainfo, GSettings schema
├── po/                         Translations of the app's own strings
├── mini-tokyo-3d.spec          RPM spec
└── build-aux/make-rpm.sh       Builds the RPM
```

## Building and installing

Install the build tools and runtime libraries:

```bash
sudo dnf install nodejs npm meson gjs gtk4 libadwaita webkitgtk6.0 libsecret \
    glib2-devel gettext libxml2 desktop-file-utils appstream rpm-build
```

### As an RPM package (recommended)

```bash
gnome/build-aux/make-rpm.sh
sudo dnf install ~/rpmbuild/RPMS/noarch/mini-tokyo-3d-*.noarch.rpm
```

The script builds the web bundle with npm, packs a source tarball and runs
`rpmbuild`. To uninstall, run `sudo dnf remove mini-tokyo-3d`.

### With Meson directly

Run these commands from the repository root:

```bash
npm install && npm run build
meson setup _build --prefix=/usr
meson compile -C _build
meson test -C _build          # validates the desktop entry, metainfo and schema
sudo meson install -C _build
```

Use `--prefix=$HOME/.local` to install for your user only, without `sudo`. To
uninstall, run `sudo ninja -C _build uninstall`.

## First launch

Mini Tokyo 3D appears in Activities as **Mini Tokyo 3D**. On first launch it
asks for a Mapbox access token, which the map needs:

1. Create a token at <https://account.mapbox.com/>. The default public token
   works. A token with URL restrictions does not, because the app has no web
   address.
2. Open **Preferences** from the main menu, or press <kbd>Ctrl</kbd>+<kbd>,</kbd>,
   and paste the token under **Mapbox**.
3. Optionally, add the tokens for the Public Transportation Open Data Center
   and the Open Data Challenge. They enable real-time data for more railways,
   like `MT3D_SECRET_ODPT` and `MT3D_SECRET_CHALLENGE` do for the web build.

The tokens are stored in your keyring; you can inspect them in **Passwords and
Keys** (Seahorse). The map reloads with the new settings when Preferences
closes.

**Language** in Preferences sets the map language. **Automatic** follows the
system language, like the web app follows the browser.

## Command line

The URL parameters of the web app are available as options, and a
minitokyo3d.com link can be opened directly:

```bash
mini-tokyo-3d --lang=ja
mini-tokyo-3d --selection=odpt.Station:JR-East.Yamanote.Tokyo
mini-tokyo-3d 'https://minitokyo3d.com/?selection=odpt.Station:JR-East.Yamanote.Tokyo#15/35.6812/139.7671/30/45'
mini-tokyo-3d --gtfs-url=URL --gtfs-color=RRGGBB [--gtfs-vehicle-position-url=URL]
```

Run `mini-tokyo-3d --help` for the full list. Options given on the command line
take precedence over the Preferences.

## Plugins

Plugins are bundled at build time, as with the `MT3D_PLUGIN_*` variables of the
web build. Pass each plugin's built file to Meson:

```bash
meson setup _build --prefix=/usr -Dplugin_precipitation=/path/to/mini-tokyo-3d-plugin-precipitation.min.js
```

The available options are `plugin_precipitation`, `plugin_fireworks`,
`plugin_livecam`, `plugin_plateau` and `plugin_gtfs`.

## Debugging

```bash
MT3D_DEVTOOLS=1 mini-tokyo-3d
```

This prints the page's console messages to the terminal and adds **Inspect
Element** to the context menu, which opens the WebKit Web Inspector.

## Differences from the web version

- **Share buttons:** these use the Web Share API, which WebKitGTK does not
  provide. As in Firefox and Chrome on Linux, the share buttons are not shown.
- **Links:** links open in your default browser.
- **Analytics:** Google Analytics is not included.
- **Translations:** the map is translated into all 11 languages as on the web.
  The app's own strings (menus, Preferences) are in English for now and can be
  translated through `gnome/po/`.
>>>>>>> Stashed changes

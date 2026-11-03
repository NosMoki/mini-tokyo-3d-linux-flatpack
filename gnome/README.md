# Mini Tokyo 3D for GNOME

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

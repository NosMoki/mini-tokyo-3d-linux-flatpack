#!/usr/bin/env bash
# Builds Mini Tokyo 3D as a Flatpak and installs it for the current user.
#
#   gnome/build-flatpak.sh               build and install
#   gnome/build-flatpak.sh --bundle      also write gnome/_build/mini-tokyo-3d.flatpak,
#                                        a single file to install on other computers
#   gnome/build-flatpak.sh --reuse-dist  don't rebuild the web bundle in dist/
#
# Needs node and npm (for the web app) and flatpak. The GNOME 51 SDK, the Rust
# SDK extension and flatpak-builder are installed from Flathub for the current
# user when they are missing.
set -euo pipefail
cd "$(dirname "$0")"

APP_ID=io.github.nosmoki.MiniTokyo3D
GNOME_VERSION=51
FREEDESKTOP_VERSION=26.08

bundle=false
web_args=()
for arg in "$@"; do
    case "$arg" in
        --bundle) bundle=true ;;
        --reuse-dist) web_args+=(--reuse-dist) ;;
        *) echo "Unknown option: $arg" >&2; exit 2 ;;
    esac
done

echo "== Staging the web app"
node build-web.mjs "${web_args[@]}"

echo "== Checking the Flatpak SDK"
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
for ref in "org.gnome.Platform//$GNOME_VERSION" "org.gnome.Sdk//$GNOME_VERSION" \
           "org.freedesktop.Sdk.Extension.rust-stable//$FREEDESKTOP_VERSION"; do
    flatpak info "$ref" >/dev/null 2>&1 || flatpak install --user --noninteractive -y flathub "$ref"
done
if command -v flatpak-builder >/dev/null; then
    builder=(flatpak-builder)
else
    flatpak info org.flatpak.Builder >/dev/null 2>&1 || flatpak install --user --noninteractive -y flathub org.flatpak.Builder
    # The builder needs no access to documents, and the portal may not be running.
    builder=(flatpak run --no-documents-portal org.flatpak.Builder)
fi

echo "== Building"
"${builder[@]}" --user --install --force-clean \
    --state-dir=_build/flatpak-builder --repo=_build/repo \
    _build/flatpak "$APP_ID.yml"

if $bundle; then
    flatpak build-bundle _build/repo _build/mini-tokyo-3d.flatpak "$APP_ID" \
        --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo
    echo "Wrote gnome/_build/mini-tokyo-3d.flatpak"
fi

echo "Installed. Start it from the app grid, or run: flatpak run $APP_ID"

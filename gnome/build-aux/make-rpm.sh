#!/bin/bash
# Builds the RPM of the GNOME desktop app.
#
#   gnome/build-aux/make-rpm.sh
#
# The packages land in ~/rpmbuild/RPMS/noarch (set RPM_TOPDIR to use another
# directory). Requires nodejs, npm, rpm-build and the BuildRequires listed in
# gnome/mini-tokyo-3d.spec.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$root"

name=mini-tokyo-3d
version=$(node -p "require('./package.json').version")
topdir=${RPM_TOPDIR:-$(rpm --eval '%{_topdir}')}

# The web bundle. npm needs the network, so it is built here rather than in
# rpmbuild.
if [ ! -d node_modules ]; then
    npm install
fi
npm run build

# The source tarball holds only what the Meson build uses.
mkdir -p "$topdir"/{BUILD,RPMS,SOURCES,SPECS,SRPMS}
tar --create --gzip --exclude-vcs \
    --file "$topdir/SOURCES/$name-$version.tar.gz" \
    --transform "s,^,$name-$version/," \
    meson.build meson.options package.json LICENSE README.md gnome \
    public/favicon-16x16.png public/favicon-32x32.png \
    public/android-chrome-192x192.png public/android-chrome-512x512.png \
    dist/mini-tokyo-3d.min.js dist/mini-tokyo-3d.min.css dist/assets

rpmbuild --define "_topdir $topdir" -ba gnome/mini-tokyo-3d.spec

echo
echo "Install with:"
echo "  sudo dnf install $topdir/RPMS/noarch/$name-$version-*.noarch.rpm"

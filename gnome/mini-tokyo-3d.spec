# RPM package of the GNOME desktop app. Build it with gnome/build-aux/make-rpm.sh,
# which builds the web bundle (dist/) with npm first: npm needs network access,
# which rpmbuild doesn't give, so the bundle comes prebuilt in the tarball.

%global app_id com.minitokyo3d.MiniTokyo3D

Name:           mini-tokyo-3d
Version:        4.1.0
Release:        1%{?dist}
Summary:        Real-time 3D map of Tokyo's public transport

License:        MIT
URL:            https://minitokyo3d.com
Source0:        %{name}-%{version}.tar.gz

BuildArch:      noarch

BuildRequires:  meson >= 1.1.0
BuildRequires:  gjs
BuildRequires:  glib2-devel
BuildRequires:  gettext
BuildRequires:  libxml2
BuildRequires:  desktop-file-utils
BuildRequires:  appstream

Requires:       gjs
Requires:       glib2
Requires:       gtk4 >= 4.18
Requires:       libadwaita >= 1.8
Requires:       webkitgtk6.0
Requires:       libsecret
Requires:       hicolor-icon-theme

%description
Mini Tokyo 3D is a real-time 3D digital map of the public transport system in
the Greater Tokyo area. Trains and aircraft move across the city based on
timetables and live operating data from the Public Transportation Open Data
Center.

This package is the GNOME desktop app. A Mapbox access token is required to
display the map.

%prep
%autosetup

%build
%meson
%meson_build

%install
%meson_install
# Once gnome/po/LINGUAS lists a language, add "%%find_lang %%{app_id}" here and
# change the %%files line below to "%%files -f %%{app_id}.lang".

%check
desktop-file-validate %{buildroot}%{_datadir}/applications/%{app_id}.desktop
appstreamcli validate --no-net %{buildroot}%{_metainfodir}/%{app_id}.metainfo.xml

%files
%license LICENSE
%doc README.md
%{_bindir}/%{name}
%{_datadir}/%{app_id}/
%{_datadir}/applications/%{app_id}.desktop
%{_datadir}/glib-2.0/schemas/%{app_id}.gschema.xml
%{_datadir}/icons/hicolor/*/apps/%{app_id}.png
%{_metainfodir}/%{app_id}.metainfo.xml

%changelog
* Wed Nov 04 2026 Moki <143357414+NosMoki@users.noreply.github.com> - 4.1.0-1
- Initial package of the GNOME desktop app

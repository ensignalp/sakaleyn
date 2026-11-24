# Sakaleyn: Fedora/COPR paketi.
# COPR'da "Enable internet access during builds" açık olmalı (cargo bağımlılıkları
# derleme sırasında indirilir) ya da kaynak arşivi `cargo vendor` ile hazırlanmalı.
# Ezan kayıtları kaynak arşivinde yoksa paket yalnızca uyarı sesleriyle gelir.

%global uuid sakaleyn@ensignalp

Name:           sakaleyn
Version:        0.1.0
Release:        1%{?dist}
Summary:        Prayer times with reminders and adhan for Linux (Diyanet)
License:        MIT
URL:            https://github.com/ensignalp/sakaleyn
Source0:        %{url}/archive/v%{version}/%{name}-%{version}.tar.gz

BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  gcc
BuildRequires:  systemd-rpm-macros
Requires:       (pipewire-utils or pulseaudio-utils or gstreamer1-plugins-base-tools or ffmpeg-free or mpv)
Requires:       geoclue2
Recommends:     gjs
Recommends:     libadwaita
Recommends:     gnome-shell-extension-sakaleyn = %{version}-%{release}

%description
Sakaleyn shows prayer times from the Presidency of Religious Affairs (Diyanet),
reminds you before each prayer (45 minutes by default, configurable per
prayer) and plays the adhan at prayer time. Location is detected automatically
with GeoClue. Friday and Ramadan (suhoor/iftar) reminders, religious days.
A small D-Bus service (`sakaleyn daemon`) does the work; a tray icon is
available for KDE, XFCE, Cinnamon and MATE.

%package -n gnome-shell-extension-sakaleyn
Summary:        GNOME Shell extension for Sakaleyn
BuildArch:      noarch
Requires:       gnome-shell >= 48
Requires:       %{name} = %{version}-%{release}

%description -n gnome-shell-extension-sakaleyn
Top bar indicator with countdown, Quick Settings tile and date menu card for Sakaleyn.

%prep
%autosetup -n %{name}-%{version}

%build
cargo build --release --locked

%install
scripts/stage.sh %{buildroot} target/release/sakaleyn %{_prefix}
rm -rf %{buildroot}%{_datadir}/licenses %{buildroot}%{_datadir}/doc

%check
cargo test --release --locked

%post
%systemd_user_post sakaleyn.service

%preun
%systemd_user_preun sakaleyn.service

%files
%license LICENSE
%doc README.md config.example.toml
%{_bindir}/sakaleyn
%{_userunitdir}/sakaleyn.service
%{_datadir}/dbus-1/services/io.github.ensignalp.Sakaleyn.service
%{_datadir}/applications/sakaleyn.desktop
%{_datadir}/icons/hicolor/*/apps/sakaleyn*
%{_datadir}/sakaleyn/
%config(noreplace) %{_sysconfdir}/xdg/autostart/sakaleyn-tray.desktop

%files -n gnome-shell-extension-sakaleyn
%license LICENSE
%{_datadir}/gnome-shell/extensions/%{uuid}/
%{_datadir}/glib-2.0/schemas/org.gnome.shell.extensions.sakaleyn.gschema.xml

%changelog
* Mon Nov 23 2026 Muhammed Alparslan <ensignalp@users.noreply.github.com> - 0.1.0-1
- First release.

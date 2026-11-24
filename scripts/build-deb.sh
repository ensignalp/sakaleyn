#!/bin/sh
# Debian/Ubuntu paketi (dpkg-deb gerekir).
#   scripts/build-deb.sh <sakaleyn-ikilisi> <mimari: x86_64|aarch64> <sürüm>
# Çıktı: dist/sakaleyn_<sürüm>_<amd64|arm64>.deb
set -eu
bin="$1"; arch="$2"; ver="$3"
case "$arch" in x86_64) darch=amd64 ;; aarch64) darch=arm64 ;; *) echo "mimari?" >&2; exit 1 ;; esac
root="$(mktemp -d)/sakaleyn_${ver}_${darch}"
scripts/stage.sh "$root" "$bin" /usr
mv "$root/usr/share/licenses/sakaleyn/LICENSE" "$root/usr/share/doc/sakaleyn/copyright"
rm -rf "$root/usr/share/licenses"
size=$(du -sk "$root/usr" | cut -f1)
mkdir -p "$root/DEBIAN"
cat > "$root/DEBIAN/control" <<CTRL
Package: sakaleyn
Version: ${ver}
Architecture: ${darch}
Maintainer: Muhammed Alparslan <ensignalp@users.noreply.github.com>
Installed-Size: ${size}
Depends: geoclue-2.0
Recommends: pipewire-bin | pulseaudio-utils | gstreamer1.0-tools | ffmpeg | mpv, gjs, gir1.2-adw-1, gnome-shell (>= 48)
Section: utils
Priority: optional
Homepage: https://github.com/ensignalp/sakaleyn
Description: Prayer times with reminders and adhan (Diyanet)
 Shows prayer times from Diyanet, reminds you before each prayer (45 minutes
 by default) and plays the adhan at prayer time. Location is detected with
 GeoClue. Includes the sakaleyn D-Bus service, a tray icon for KDE/XFCE and
 the GNOME Shell extension (sakaleyn@ensignalp).
CTRL
cat > "$root/DEBIAN/postinst" <<'POST'
#!/bin/sh
set -e
if [ "$1" = configure ]; then
    command -v glib-compile-schemas >/dev/null && glib-compile-schemas /usr/share/glib-2.0/schemas || true
    command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q -t /usr/share/icons/hicolor || true
fi
POST
cat > "$root/DEBIAN/postrm" <<'POST'
#!/bin/sh
set -e
command -v glib-compile-schemas >/dev/null && glib-compile-schemas /usr/share/glib-2.0/schemas || true
POST
chmod 755 "$root/DEBIAN/postinst" "$root/DEBIAN/postrm"
mkdir -p dist
dpkg-deb --root-owner-group --build "$root" "dist/sakaleyn_${ver}_${darch}.deb" >/dev/null
rm -rf "$(dirname "$root")"
echo "dist/sakaleyn_${ver}_${darch}.deb"

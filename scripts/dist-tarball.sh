#!/bin/sh
# Sürüm paketleri (GitHub release):
#   dist/sakaleyn-<mimari>-linux.tar.gz  statik ikili + install.sh + veri + eklenti + sesler
#   dist/sakaleyn-sounds.tar.gz          ezan kayıtları (kaynaktan derleyen mimariler için)
#
#   scripts/dist-tarball.sh <statik-ikili> <mimari: x86_64|aarch64> <sürüm>
# Ezan kayıtları depoda yoktur (.gitignore); önce `make sounds` ile hazırlanmalıdır.
set -eu
bin="$1"; arch="$2"; ver="$3"
name="sakaleyn-${ver}"
root="$(mktemp -d)"
d="$root/$name"
mkdir -p "$d/bin" "$d/assets" "$d/scripts" dist
install -m755 "$bin" "$d/bin/sakaleyn"
cp -r assets/logo assets/sounds "$d/assets/"
cp -r packaging gnome-extension settings-app "$d/"
rm -f "$d/gnome-extension/sakaleyn@ensignalp/schemas/gschemas.compiled"
cp install.sh README.md LICENSE config.example.toml "$d/"
cp scripts/enable-extension.sh "$d/scripts/"
n=$(find "$d/assets/sounds/adhan" -name '*.ogg' | wc -l)
[ "$n" -gt 0 ] || echo "uyarı: ezan sesi yok (make sounds)" >&2
tar -C "$root" -czf "dist/sakaleyn-${arch}-linux.tar.gz" "$name"
(cd assets/sounds && tar -czf ../../dist/sakaleyn-sounds.tar.gz adhan/*.ogg alerts/uyandirma.ogg)
rm -rf "$root"
ls -la dist/sakaleyn-${arch}-linux.tar.gz dist/sakaleyn-sounds.tar.gz

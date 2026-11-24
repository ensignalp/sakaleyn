#!/bin/sh
# Sistem düzeninde (/usr) bütün dosyaları DESTDIR altına yerleştirir.
# RPM, .deb ve Arch paketleri bunu kullanır.
#   scripts/stage.sh <DESTDIR> <sakaleyn-ikilisi> [PREFIX=/usr]
set -eu
dest="$1"; bin="$2"; prefix="${3:-/usr}"
uuid="sakaleyn@ensignalp"
ext="gnome-extension/$uuid"
share="$dest$prefix/share"

install -Dm755 "$bin" "$dest$prefix/bin/sakaleyn"
sed "s|@BINDIR@|$prefix/bin|g" packaging/systemd/sakaleyn.service \
    | install -Dm644 /dev/stdin "$dest$prefix/lib/systemd/user/sakaleyn.service"
sed "s|@BINDIR@|$prefix/bin|g" packaging/dbus/io.github.ensignalp.Sakaleyn.service \
    | install -Dm644 /dev/stdin "$share/dbus-1/services/io.github.ensignalp.Sakaleyn.service"
sed "s|@BINDIR@|$prefix/bin|g" packaging/applications/sakaleyn.desktop \
    | install -Dm644 /dev/stdin "$share/applications/sakaleyn.desktop"
sed "s|@BINDIR@|$prefix/bin|g" packaging/autostart/sakaleyn-tray.desktop \
    | install -Dm644 /dev/stdin "$dest/etc/xdg/autostart/sakaleyn-tray.desktop"

# sesler (ezan kayıtları varsa; telif nedeniyle depoda yok, bkz. README)
install -d "$share/sakaleyn/sounds/adhan" "$share/sakaleyn/sounds/alerts"
install -m644 assets/sounds/sounds.json "$share/sakaleyn/sounds/"
for f in assets/sounds/adhan/*.ogg assets/sounds/alerts/*.ogg; do
    [ -f "$f" ] && install -m644 "$f" "$share/sakaleyn/sounds/$(basename "$(dirname "$f")")/"
done

# simgeler
install -Dm644 assets/logo/sakaleyn.svg "$share/icons/hicolor/scalable/apps/sakaleyn.svg"
install -Dm644 assets/logo/sakaleyn-symbolic.svg "$share/icons/hicolor/symbolic/apps/sakaleyn-symbolic.svg"
for s in 16 32 48 128 256 512; do
    install -Dm644 "assets/logo/sakaleyn-$s.png" "$share/icons/hicolor/${s}x${s}/apps/sakaleyn.png"
done

# bağımsız ayarlar penceresi (eklentiyle aynı arayüz)
install -Dm644 settings-app/main.js "$share/sakaleyn/settings/main.js"
install -Dm644 "$ext/settings-ui.js" "$share/sakaleyn/settings/settings-ui.js"
install -Dm644 "$ext/lib/client.js" "$share/sakaleyn/settings/lib/client.js"
install -Dm644 "$ext/lib/i18n.js" "$share/sakaleyn/settings/lib/i18n.js"

# GNOME eklentisi (şema sistem şemalarına)
install -d "$share/gnome-shell/extensions/$uuid"
cp -r "$ext/." "$share/gnome-shell/extensions/$uuid/"
rm -rf "$share/gnome-shell/extensions/$uuid/schemas"
install -Dm644 "$ext/schemas/org.gnome.shell.extensions.sakaleyn.gschema.xml" \
    "$share/glib-2.0/schemas/org.gnome.shell.extensions.sakaleyn.gschema.xml"

install -Dm644 LICENSE "$share/licenses/sakaleyn/LICENSE"
install -Dm644 README.md "$share/doc/sakaleyn/README.md"
install -Dm644 config.example.toml "$share/doc/sakaleyn/config.example.toml"

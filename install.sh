#!/bin/sh
# Sakaleyn: her Linux dağıtımı için tek komutla kurulum (sudo gerekmez).
#
#   curl -fsSL https://raw.githubusercontent.com/ensignalp/sakaleyn/main/install.sh | sh
#
# Depo ya da sürüm arşivi içinden:  ./install.sh
#
# Seçenekler:
#   --uninstall        kaldır (ayarlar ~/.config/sakaleyn kalır)
#   --prefix DİZİN     ikili için önek; varsayılan ~/.local (ikili: DİZİN/bin/sakaleyn)
#   --version SÜRÜM    belirli bir sürüm (ör. 0.1.0); varsayılan en yenisi
#   --bin DOSYA        indirmek yerine bu ikiliyi kur (make install bunu kullanır)
#   --tray             GNOME dışında da tepsi simgesini oturum açılışına ekle (KDE vb. otomatik)
#   --no-start         servisi etkinleştirme/başlatma (paketleme, test)
#
# Kurulanlar: ~/.local/bin/sakaleyn (statik, kütüphane bağımlılığı yok), systemd
# kullanıcı birimi (systemd varsa), D-Bus etkinleştirme dosyası, sesler, simgeler,
# ayarlar penceresi ve GNOME Shell eklentisi (GNOME varsa).

set -eu

REPO="ensignalp/sakaleyn"
UUID="sakaleyn@ensignalp"
BUS="io.github.ensignalp.Sakaleyn"
PREFIX="${HOME}/.local"
VERSION=""
BIN=""
ACTION="install"
TRAY=""
START=1

while [ $# -gt 0 ]; do
    case "$1" in
        --uninstall) ACTION="uninstall" ;;
        --prefix) PREFIX="$2"; shift ;;
        --version) VERSION="${2#v}"; shift ;;
        --bin) BIN="$2"; shift ;;
        --tray) TRAY=1 ;;
        --no-start) START="" ;;
        -h|--help) sed -n '2,20p' "$0"; exit 0 ;;
        *) echo "bilinmeyen seçenek: $1" >&2; exit 2 ;;
    esac
    shift
done

BINDIR="${PREFIX}/bin"
DATADIR="${XDG_DATA_HOME:-${HOME}/.local/share}"
CONFDIR="${XDG_CONFIG_HOME:-${HOME}/.config}"
EXT_DIR="${DATADIR}/gnome-shell/extensions/${UUID}"
DBUS_DIR="${DATADIR}/dbus-1/services"
UNIT_DIR="${CONFDIR}/systemd/user"
SHARE="${DATADIR}/sakaleyn"

say() { printf '\033[1;32m==>\033[0m \033[1m%s\033[0m\n' "$*"; }
warn() { printf '\033[1;33m!!\033[0m %s\n' "$*" >&2; }
die() { printf '\033[1;31mhata:\033[0m %s\n' "$*" >&2; exit 1; }
have() { command -v "$1" >/dev/null 2>&1; }
has_systemd() { have systemctl && systemctl --user show-environment >/dev/null 2>&1; }
is_gnome() { have gnome-shell || case "${XDG_CURRENT_DESKTOP:-}" in *GNOME*) true ;; *) false ;; esac; }

[ "$(id -u)" != 0 ] || die "Sakaleyn kullanıcı başına kurulur; sudo olmadan çalıştırın."

uninstall() {
    if has_systemd; then
        systemctl --user disable --now sakaleyn.service 2>/dev/null || true
    fi
    have gnome-extensions && gnome-extensions disable "$UUID" 2>/dev/null || true
    pkill -x -u "$(id -u)" sakaleyn 2>/dev/null || true
    rm -f "${BINDIR}/sakaleyn" "${UNIT_DIR}/sakaleyn.service" "${DBUS_DIR}/${BUS}.service"
    rm -f "${DATADIR}/applications/sakaleyn.desktop" "${CONFDIR}/autostart/sakaleyn-tray.desktop"
    rm -f "${DATADIR}/icons/hicolor/scalable/apps/sakaleyn.svg" "${DATADIR}/icons/hicolor/symbolic/apps/sakaleyn-symbolic.svg"
    for s in 16 32 48 128 256 512; do rm -f "${DATADIR}/icons/hicolor/${s}x${s}/apps/sakaleyn.png"; done
    rm -rf "$EXT_DIR" "$SHARE"
    has_systemd && systemctl --user daemon-reload || true
    say "Sakaleyn kaldırıldı. Ayarlar (~/.config/sakaleyn) ve önbellek (~/.cache/sakaleyn) duruyor."
}

if [ "$ACTION" = uninstall ]; then
    uninstall
    exit 0
fi

# ------------------------------------------------------------------ kaynak
TMP=""
cleanup() { [ -n "$TMP" ] && rm -rf "$TMP"; }
trap cleanup EXIT INT TERM

download() { # url hedef
    if have curl; then curl -fsSL "$1" -o "$2"
    elif have wget; then wget -qO "$2" "$1"
    else die "curl ya da wget gerekli."; fi
}

SRC=""
HERE="$(cd "$(dirname "$0")" 2>/dev/null && pwd || true)"
if [ -n "$HERE" ] && [ -d "$HERE/gnome-extension/$UUID" ] && [ -f "$HERE/assets/sounds/sounds.json" ]; then
    SRC="$HERE" # depo ya da açılmış sürüm arşivi
    [ -z "$BIN" ] && [ -x "$SRC/bin/sakaleyn" ] && BIN="$SRC/bin/sakaleyn"
fi

if [ -z "$SRC" ] || [ -z "$BIN" ]; then
    arch="$(uname -m)"
    case "$arch" in
        x86_64|amd64) arch=x86_64 ;;
        *) arch="" ;;
    esac
    TMP="$(mktemp -d)"
    if [ -n "$VERSION" ]; then base="https://github.com/${REPO}/releases/download/v${VERSION}"
    else base="https://github.com/${REPO}/releases/latest/download"; fi
    if [ -n "$arch" ]; then
        say "Sakaleyn indiriliyor (${VERSION:-son sürüm}, $arch)…"
        download "${base}/sakaleyn-${arch}-linux.tar.gz" "$TMP/sakaleyn.tar.gz" ||
            die "sürüm arşivi indirilemedi: ${base}/sakaleyn-${arch}-linux.tar.gz"
        tar -xzf "$TMP/sakaleyn.tar.gz" -C "$TMP"
        SRC="$(find "$TMP" -mindepth 1 -maxdepth 1 -type d | head -1)"
        BIN="$SRC/bin/sakaleyn"
    else
        # hazır ikili olmayan mimari: kaynaktan derle, sesleri sürümden al
        have cargo || die "$(uname -m) için hazır ikili yok; derlemek için Rust (cargo) kurun: https://rustup.rs"
        have git || die "kaynaktan derlemek için git gerekli."
        say "$(uname -m) için kaynaktan derleniyor (birkaç dakika)…"
        git clone --depth 1 ${VERSION:+--branch "v$VERSION"} "https://github.com/${REPO}.git" "$TMP/src" >/dev/null 2>&1
        (cd "$TMP/src" && cargo build --release --locked)
        SRC="$TMP/src"
        BIN="$SRC/target/release/sakaleyn"
        if download "${base}/sakaleyn-sounds.tar.gz" "$TMP/sounds.tar.gz"; then
            tar -xzf "$TMP/sounds.tar.gz" -C "$SRC/assets/sounds"
        else
            warn "ezan sesleri indirilemedi; uyarı sesleri ve bildirimler çalışır."
        fi
    fi
fi
[ -x "$BIN" ] || die "ikili bulunamadı: $BIN"

# ------------------------------------------------------------------ kurulum
say "Kuruluyor: $BINDIR/sakaleyn"
install -Dm755 "$BIN" "$BINDIR/sakaleyn"

subst() { sed "s|@BINDIR@|$BINDIR|g" "$1"; }
mkdir -p "$UNIT_DIR" "$DBUS_DIR" "$DATADIR/applications"
subst "$SRC/packaging/systemd/sakaleyn.service" > "$UNIT_DIR/sakaleyn.service"
subst "$SRC/packaging/dbus/${BUS}.service" > "$DBUS_DIR/${BUS}.service"
subst "$SRC/packaging/applications/sakaleyn.desktop" > "$DATADIR/applications/sakaleyn.desktop"

# sesler
rm -rf "$SHARE/sounds"
mkdir -p "$SHARE/sounds/adhan" "$SHARE/sounds/alerts"
cp "$SRC/assets/sounds/sounds.json" "$SHARE/sounds/"
n_adhan=0
for f in "$SRC"/assets/sounds/adhan/*.ogg; do [ -f "$f" ] && cp "$f" "$SHARE/sounds/adhan/" && n_adhan=$((n_adhan + 1)); done
for f in "$SRC"/assets/sounds/alerts/*.ogg; do [ -f "$f" ] && cp "$f" "$SHARE/sounds/alerts/"; done

# simgeler
L="$SRC/assets/logo"
install -Dm644 "$L/sakaleyn.svg" "$DATADIR/icons/hicolor/scalable/apps/sakaleyn.svg"
install -Dm644 "$L/sakaleyn-symbolic.svg" "$DATADIR/icons/hicolor/symbolic/apps/sakaleyn-symbolic.svg"
for s in 16 32 48 128 256 512; do
    install -Dm644 "$L/sakaleyn-$s.png" "$DATADIR/icons/hicolor/${s}x${s}/apps/sakaleyn.png"
done
have gtk-update-icon-cache && gtk-update-icon-cache -q -t "$DATADIR/icons/hicolor" 2>/dev/null || true
have update-desktop-database && update-desktop-database -q "$DATADIR/applications" 2>/dev/null || true

# ayarlar penceresi (eklentiyle aynı arayüz; GNOME dışı masaüstleri için)
E="$SRC/gnome-extension/$UUID"
mkdir -p "$SHARE/settings/lib"
cp "$SRC/settings-app/main.js" "$E/settings-ui.js" "$SHARE/settings/"
cp "$E/lib/client.js" "$E/lib/i18n.js" "$SHARE/settings/lib/"

# GNOME eklentisi
ext_note=""
if is_gnome; then
    rm -rf "$EXT_DIR"
    mkdir -p "$EXT_DIR"
    cp -r "$E/." "$EXT_DIR/"
    if have glib-compile-schemas; then
        glib-compile-schemas "$EXT_DIR/schemas"
    else
        warn "glib-compile-schemas yok; eklenti ayarları yüklenemeyebilir."
    fi
    if [ -n "$START" ]; then
        if gnome-extensions enable "$UUID" 2>/dev/null; then
            ext_note="GNOME eklentisi etkin."
        elif have gsettings; then
            cur="$(gsettings get org.gnome.shell enabled-extensions 2>/dev/null || echo '[]')"
            case "$cur" in
                *"'$UUID'"*) ;;
                "@as []"|"[]") gsettings set org.gnome.shell enabled-extensions "['$UUID']" ;;
                *) gsettings set org.gnome.shell enabled-extensions "${cur%]}, '$UUID']" ;;
            esac
            ext_note="GNOME eklentisi etkinleştirildi; görünmesi için oturumu kapatıp yeniden açın."
        fi
    fi
fi

# tepsi simgesi (KDE, XFCE, Cinnamon, MATE, Budgie, LXQt)
if [ -n "$TRAY" ] || ! is_gnome; then
    mkdir -p "$CONFDIR/autostart"
    subst "$SRC/packaging/autostart/sakaleyn-tray.desktop" | sed '/^OnlyShowIn=/d' > "$CONFDIR/autostart/sakaleyn-tray.desktop"
fi

# servis
if [ -n "$START" ]; then
    if has_systemd; then
        systemctl --user daemon-reload
        systemctl --user enable sakaleyn.service >/dev/null 2>&1
        systemctl --user restart sakaleyn.service
    else
        warn "systemd yok: servis ilk kullanımda D-Bus tarafından başlatılacak."
    fi
    if ! is_gnome && [ -n "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ]; then
        (nohup "$BINDIR/sakaleyn" tray >/dev/null 2>&1 &) || true
    fi
fi

# ------------------------------------------------------------------ denetim
player=""
for p in pw-play paplay gst-play-1.0 ffplay mpv; do have "$p" && { player="$p"; break; }; done
[ -n "$player" ] || warn "ses oynatıcı bulunamadı; pipewire-utils (pw-play), pulseaudio-utils, gstreamer, ffmpeg ya da mpv kurun."
have gjs || warn "ayarlar penceresi için gjs ve libadwaita gerekli (GNOME'da kurulu gelir)."
[ "$n_adhan" -gt 0 ] || warn "ezan sesi yok; ~/.local/share/sakaleyn/sounds/ klasörüne kendi ezan dosyalarınızı koyabilirsiniz."
case ":$PATH:" in *":$BINDIR:"*) ;; *) warn "$BINDIR PATH'te değil; komut için tam yolu kullanın: $BINDIR/sakaleyn" ;; esac

echo
say "Sakaleyn kuruldu."
[ -n "$ext_note" ] && echo "    $ext_note"
echo "    Vakitler:  sakaleyn today        Ayarlar:  sakaleyn settings"
echo "    Durum:     systemctl --user status sakaleyn"
echo "    Kaldırma:  curl -fsSL https://raw.githubusercontent.com/${REPO}/main/install.sh | sh -s -- --uninstall"

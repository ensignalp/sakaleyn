#!/usr/bin/env bash
# Ezan kayıtlarını uygulamaya hazırlar: baş/son sessizliği kırpar, ses yüksekliğini
# EBU R128 −16 LUFS'a eşitler, 2 sn yumuşak giriş + 1,5 sn çıkış ekler, Ogg Vorbis'e çevirir.
#
#   scripts/prepare-sounds.sh [kaynak-klasör] [hedef-klasör]
#   varsayılan: "../ezan sesleri" → assets/sounds/adhan
#
# Kayıtların telif durumu belirsiz olduğu için çıktı depoya girmez (.gitignore);
# kurulum paketine konur. Dosyalar ada göre değil anahtar kelimeye göre bulunur.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC="${1:-$ROOT/../ezan sesleri}"
DST="${2:-$ROOT/assets/sounds/adhan}"
ALERTS="$ROOT/assets/sounds/alerts"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

command -v ffmpeg >/dev/null || { echo "ffmpeg gerekli" >&2; exit 1; }
mkdir -p "$DST" "$ALERTS"

# kimlik | dosya adında geçen anahtar kelime | hedef klasör
MAP=(
  "alafasy-fajr|Alafasy|$DST"
  "kassas-medine|Qassas|$DST"
  "qatami|Qatami|$DST"
  "kabe|Kabe|$DST"
  "cosar-rast|Rast Makam|$DST"
  "cosar-ussak|Makamı Öğle|$DST"
  "senol-ogle|Şenol|$DST"
  "uyandirma|Esselatu|$ALERTS"
)

find_src() {
  local key="$1" f
  for f in "$SRC"/*; do
    case "$(basename "$f")" in *"$key"*) printf '%s\n' "$f"; return 0 ;; esac
  done
  return 1
}

for entry in "${MAP[@]}"; do
  IFS='|' read -r id key out <<<"$entry"
  if ! in="$(find_src "$key")"; then
    echo "atlandı: '$key' içeren dosya yok ($id)" >&2
    continue
  fi
  trim="silenceremove=start_periods=1:start_threshold=-45dB:start_silence=0.3"
  ffmpeg -nostdin -hide_banner -loglevel error -y -i "$in" -vn \
    -af "$trim,areverse,$trim,areverse,loudnorm=I=-16:TP=-1.5:LRA=11" \
    -ar 44100 -ac 2 "$TMP/$id.wav"
  dur="$(ffprobe -v error -show_entries format=duration -of csv=p=0 "$TMP/$id.wav")"
  out_start="$(awk -v d="$dur" 'BEGIN{printf "%.2f", (d > 4 ? d - 1.5 : d)}')"
  fade_in=2
  [ "$id" = uyandirma ] && fade_in=0.5
  ffmpeg -nostdin -hide_banner -loglevel error -y -i "$TMP/$id.wav" \
    -af "afade=t=in:d=$fade_in,afade=t=out:st=$out_start:d=1.5" \
    -c:a libvorbis -q:a 4 "$out/$id.ogg"
  printf '%-14s %6.1f sn  %s\n' "$id" "$dur" "$(du -h "$out/$id.ogg" | cut -f1)"
done

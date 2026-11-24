#!/usr/bin/env bash
# Ön uyarı seslerini sox ile sıfırdan sentezler (telifsiz, depoya girer).
#
#   scripts/make-alerts.sh [hedef-klasör]     varsayılan: assets/sounds/alerts
#
#   nur    yumuşak, üç notalı çan (varsayılan)       ~4 sn
#   seher  yavaş yükselen sıcak akor                  ~6 sn
#   damla  iki su damlası                             ~1,5 sn
#   hilal  iki tonlu, belirgin zil                    ~3 sn
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DST="${1:-$ROOT/assets/sounds/alerts}"
T="$(mktemp -d)"
trap 'rm -rf "$T"' EXIT
command -v sox >/dev/null || { echo "sox gerekli" >&2; exit 1; }
mkdir -p "$DST"
R=44100

# Çan: temel + 2,76x (çan tınısı) + 5,4x kısmi; üst kısmiler daha hızlı söner.
bell() { # çıktı frekans süre
  local out="$1" f="$2" d="$3"
  sox -n -r $R -c 1 "$T/p1.wav" synth "$d" sine "$f" fade l 0.004 "$d" "$d" vol 0.55
  sox -n -r $R -c 1 "$T/p2.wav" synth "$d" sine "$(awk -v f="$f" 'BEGIN{print f*2.76}')" \
    fade l 0.002 "$(awk -v d="$d" 'BEGIN{print d*0.45}')" "$(awk -v d="$d" 'BEGIN{print d*0.45}')" vol 0.22
  sox -n -r $R -c 1 "$T/p3.wav" synth "$d" sine "$(awk -v f="$f" 'BEGIN{print f*5.40}')" \
    fade l 0.001 "$(awk -v d="$d" 'BEGIN{print d*0.18}')" "$(awk -v d="$d" 'BEGIN{print d*0.18}')" vol 0.08
  sox -m "$T/p1.wav" "$T/p2.wav" "$T/p3.wav" "$out"
}

finish() { # girdi kimlik
  sox "$1" -r $R -c 2 "$T/st.wav" reverb 45 50 80 gain -n -3
  sox "$T/st.wav" -C 5 "$DST/$2.ogg"
  printf '%-6s %s sn\n' "$2" "$(soxi -D "$DST/$2.ogg")"
}

# nur: E5 – G#5 – B5 arpej, 0,35 sn aralıkla
bell "$T/n1.wav" 659.25 3.2
bell "$T/n2.wav" 830.61 3.0
bell "$T/n3.wav" 987.77 3.0
sox "$T/n2.wav" "$T/n2p.wav" pad 0.35
sox "$T/n3.wav" "$T/n3p.wav" pad 0.70
sox -m "$T/n1.wav" "$T/n2p.wav" "$T/n3p.wav" "$T/nur.wav" pad 0 0.4
finish "$T/nur.wav" nur

# seher: A3–C#4–E4–A4 akoru, 2,5 sn yavaş giriş, hafif titreşim
sox -n -r $R -c 1 "$T/seher.wav" synth 6 sine 220 sine 277.18 sine 329.63 sine 440 \
  remix - tremolo 4.5 15 fade q 2.5 6 2.5 gain -n -6
finish "$T/seher.wav" seher

# damla: hızla düşen kısa iki ton
sox -n -r $R -c 1 "$T/d1.wav" synth 0.14 sine 1500-520 fade l 0.002 0.14 0.12
sox -n -r $R -c 1 "$T/d2.wav" synth 0.14 sine 1250-430 fade l 0.002 0.14 0.12 pad 0.28
sox -m "$T/d1.wav" "$T/d2.wav" "$T/damla.wav" pad 0 1.0
finish "$T/damla.wav" damla

# hilal: B5 → G5 iki tonlu zil
bell "$T/h1.wav" 987.77 2.2
bell "$T/h2.wav" 783.99 2.6
sox "$T/h2.wav" "$T/h2p.wav" pad 0.55
sox -m "$T/h1.wav" "$T/h2p.wav" "$T/hilal.wav"
finish "$T/hilal.wav" hilal

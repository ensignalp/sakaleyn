#!/usr/bin/env python3
"""potrace çıktısından Sakaleyn logo SVG'lerini üretir.

Kullanım: python3 -I scripts/make-logo.py <potrace.svg> <çıktı-klasörü>
Üretilenler:
  sakaleyn.svg           uygulama simgesi: yeşil yuvarlak kare, krem işaret
  sakaleyn-mark.svg      saydam zeminde yeşil işaret (açık temalar)
  sakaleyn-symbolic.svg  16x16 tek renk; GNOME panel/Hızlı Ayarlar renklendirir
"""

import re
import sys
from pathlib import Path

GREEN = "#176A52"
CREAM = "#ECEECC"


def load(src: str):
    s = Path(src).read_text()
    vb = [float(x) for x in re.search(r'viewBox="([^"]+)"', s).group(1).split()]
    g = re.search(r'<g transform="([^"]+)"', s).group(1)
    paths = re.findall(r'<path d="([^"]+)"', s, re.S)
    d = " ".join(" ".join(p.split()) for p in paths)
    return vb[2], vb[3], g, d


def placed(w, h, g, d, box, height_ratio, fill):
    """İşareti box×box kutunun ortasına, yüksekliği box*height_ratio olacak şekilde koyar."""
    scale = box * height_ratio / h
    tx = (box - w * scale) / 2
    ty = (box - h * scale) / 2
    return (
        f'<g transform="translate({tx:.3f} {ty:.3f}) scale({scale:.5f})">'
        f'<g transform="{g}"><path fill="{fill}" fill-rule="evenodd" d="{d}"/></g></g>'
    )


def main(src: str, out: str) -> None:
    w, h, g, d = load(src)
    o = Path(out)
    o.mkdir(parents=True, exist_ok=True)
    head = '<svg xmlns="http://www.w3.org/2000/svg" width="{0}" height="{0}" viewBox="0 0 {0} {0}">'

    (o / "sakaleyn.svg").write_text(
        head.format(256)
        + f'<rect width="256" height="256" rx="56" fill="{GREEN}"/>'
        + placed(w, h, g, d, 256, 0.56, CREAM)
        + "</svg>\n"
    )
    (o / "sakaleyn-mark.svg").write_text(
        head.format(256) + placed(w, h, g, d, 256, 0.92, GREEN) + "</svg>\n"
    )
    (o / "sakaleyn-symbolic.svg").write_text(
        head.format(16) + placed(w, h, g, d, 16, 0.94, "#2e3436") + "</svg>\n"
    )


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])

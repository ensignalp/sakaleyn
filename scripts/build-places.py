#!/usr/bin/env python3
"""Türkiye semt/ilçe/köy dizinini üretir: assets/places.json.

Diyanet her ilçe için ayrı tablo yayınlamaz (ör. İstanbul'da 19 ilçe var;
Üsküdar, Kadıköy, Beylerbeyi "İSTANBUL" tablosunu kullanır). Bu dizin her yeri
Diyanet'in kuralıyla bir Diyanet ilçesine bağlar:
  yerin ilçesi (GeoNames admin2) Diyanet listesindeyse o ilçe, değilse il merkezi.
Arama ("üsküdar", "beylerbeyi") ve otomatik konum (en yakın semt) bunu kullanır.

Kullanım:
  python3 -I scripts/build-places.py <GeoNames TR.txt> assets/districts.json assets/places.json
GeoNames verisi CC BY 4.0 (geonames.org).
"""

import json
import sys
import unicodedata
from pathlib import Path

TR_FOLD = str.maketrans("İIıŞşĞğÜüÖöÇçÂâÎîÛû", "iiissgguuooccaaiiuu")
# Diyanet adı → GeoNames ADM2 adı (yazımı farklı olanlar)
ALIASES = {"mereglisi": "marmaraereglisi", "karadenizeregli": "eregli"}
# Yer türleri: 0 ilçe (ADM2), 1 semt/merkez (PPLX, PPLA*, PPLC), 2 köy/kasaba (PPL)
KINDS = {"ADM2": 0, "PPLX": 1, "PPLA": 1, "PPLA2": 1, "PPLA3": 1, "PPLA4": 1, "PPLC": 1, "PPL": 2}


def norm(s: str) -> str:
    s = s.translate(TR_FOLD).lower()
    s = unicodedata.normalize("NFKD", s)
    s = "".join(c for c in s if c.isalnum())
    for suffix in ("ilcesi", "merkez", "district"):
        if s.endswith(suffix) and len(s) > len(suffix) + 2:
            s = s[: -len(suffix)]
    return s


def variants(name: str):
    out = {norm(name), norm(name.split("(")[0])}
    return {ALIASES.get(v, v) for v in out}


def main(tr_txt: str, districts_json: str, out: str) -> None:
    recs = []
    with open(tr_txt, encoding="utf-8") as f:
        for line in f:
            c = line.rstrip("\n").split("\t")
            if c[6] not in ("A", "P") or c[7] not in KINDS and c[7] != "ADM1":
                continue
            recs.append(
                {
                    "name": c[1],
                    "alt": [a for a in c[3].split(",") if a],
                    "lat": float(c[4]),
                    "lon": float(c[5]),
                    "fcode": c[7],
                    "a1": c[10],
                    "a2": c[11],
                    "pop": int(c[14] or 0),
                }
            )
    districts = [d for d in json.loads(Path(districts_json).read_text(encoding="utf-8")) if d["cc"] == "TR"]

    # GeoNames il kodu ↔ Diyanet il adı
    a1_of_state = {}
    for r in recs:
        if r["fcode"] == "ADM1":
            for n in [r["name"], *r["alt"]]:
                a1_of_state.setdefault(norm(n), r["a1"])
    il_name = {}  # a1 → Diyanet il adı
    merkez = {}  # a1 → il merkezi Diyanet kimliği
    adm2_to_id = {}  # (a1, a2) → Diyanet kimliği
    adm2 = [r for r in recs if r["fcode"] == "ADM2"]
    for d in districts:
        a1 = a1_of_state.get(norm(d["state"]))
        if a1 is None:
            print(f"il eşleşmedi: {d['state']}", file=sys.stderr)
            continue
        il_name[a1] = d["state"]
        if norm(d["name"]) == norm(d["state"]):
            merkez[a1] = d["id"]
        vs = variants(d["name"])
        for r in adm2:
            if r["a1"] == a1 and variants(r["name"]) & vs:
                adm2_to_id.setdefault((a1, r["a2"]), d["id"])
    missing_merkez = sorted(set(il_name) - set(merkez))
    if missing_merkez:
        print(f"il merkezi bulunamayan iller: {missing_merkez}", file=sys.stderr)

    ils = sorted(set(il_name.values()))
    il_idx = {n: i for i, n in enumerate(ils)}
    places = []
    unmapped = 0
    for r in recs:
        if r["fcode"] == "ADM1" or r["a1"] not in il_name:
            continue
        did = adm2_to_id.get((r["a1"], r["a2"])) or merkez.get(r["a1"])
        if did is None:
            unmapped += 1
            continue
        name = r["name"].removesuffix(" İlçesi").removesuffix(" Merkez").strip()
        places.append(
            [name, il_idx[il_name[r["a1"]]], round(r["lat"], 4), round(r["lon"], 4), KINDS[r["fcode"]], r["pop"], did]
        )
    # aynı il + ad + tür tekrarlarını at (en kalabalık kalsın)
    places.sort(key=lambda p: (p[1], p[0], p[4], -p[5]))
    dedup, seen = [], set()
    for p in places:
        k = (p[1], p[0], p[4], round(p[2], 2), round(p[3], 2))
        if k not in seen:
            seen.add(k)
            dedup.append(p)
    Path(out).write_text(json.dumps({"il": ils, "p": dedup}, ensure_ascii=False, separators=(",", ":")), encoding="utf-8")
    kinds = [sum(1 for p in dedup if p[4] == k) for k in (0, 1, 2)]
    print(
        f"{len(dedup)} yer (ilçe {kinds[0]}, semt/merkez {kinds[1]}, köy/kasaba {kinds[2]}); "
        f"Diyanet ilçesine doğrudan eşlenen ADM2: {len(adm2_to_id)}; eşlenemeyen {unmapped}",
        file=sys.stderr,
    )


if __name__ == "__main__":
    main(*sys.argv[1:4])

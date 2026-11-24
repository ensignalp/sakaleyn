#!/usr/bin/env python3
"""Diyanet ilçe ağacına koordinat ekleyip assets/districts.json üretir.

Kullanım:
  python3 -I scripts/build-districts.py <diyanet-tree.json> <countries-en.json> \
      <geonames-klasörü> <çıktı.json> [--nominatim] [--cache <nominatim-cache.json>]

Koordinat kaynakları (sırayla):
  1. GeoNames TR.txt ADM2 (Türkiye ilçeleri), il merkezi için PPLA/PPLC
  2. GeoNames cities500.txt (yurt dışı şehirler)
  3. --nominatim: kalanlar için OpenStreetMap Nominatim (saniyede 1 istek)
Saat dilimi: aynı ülkedeki en yakın GeoNames şehrinin saat dilimi (yurt dışı vakitleri
Diyanet o şehrin yerel saatiyle verir).
Nominatim sonuçları ilçe kimliğiyle --cache dosyasında saklanır; tekrar çalıştırmak ucuzdur.
GeoNames verisi CC BY 4.0 (geonames.org), OSM verisi ODbL.
"""

import json
import math
import sys
import time
import unicodedata
import urllib.parse
import urllib.request
from collections import defaultdict
from pathlib import Path

TR_FOLD = str.maketrans("İIıŞşĞğÜüÖöÇçÂâÎîÛû", "iiissgguuooccaaiiuu")


def norm(s: str) -> str:
    s = s.translate(TR_FOLD).lower()
    s = unicodedata.normalize("NFKD", s)
    s = "".join(c for c in s if c.isalnum())
    for suffix in ("ilcesi", "merkez", "district", "province"):
        if s.endswith(suffix) and len(s) > len(suffix) + 2:
            s = s[: -len(suffix)]
    return s


def read_geonames(path: Path):
    with path.open(encoding="utf-8") as f:
        for line in f:
            c = line.rstrip("\n").split("\t")
            yield {
                "names": {c[1], c[2], *c[3].split(",")} - {""},
                "lat": float(c[4]),
                "lon": float(c[5]),
                "fcode": c[7],
                "cc": c[8],
                "admin1": c[10],
                "pop": int(c[14] or 0),
                "tz": c[17],
            }


def index(records):
    idx = defaultdict(list)
    for r in records:
        for n in r["names"]:
            idx[norm(n)].append(r)
    return idx


def variants(name: str):
    """'BUHL(BADEN-BADEN)' → ['buhlbadenbaden', 'buhl']; 'M.EREĞLİSİ' gibi takma adlar."""
    out = [norm(name)]
    base = name.split("(")[0].strip(" -")
    if base and norm(base) not in out:
        out.append(norm(base))
    alias = ALIASES.get(norm(name)) or ALIASES.get(norm(base))
    if alias:
        out.insert(0, norm(alias))
    return out


def best(cands, admin1=None):
    if not cands:
        return None
    rank = {"PPLC": 3, "PPLA": 2, "ADM2": 2, "PPLA2": 1}
    return max(
        cands,
        key=lambda r: (
            admin1 is not None and r["admin1"] == admin1,
            rank.get(r["fcode"], 0),
            r["pop"],
        ),
    )


def nominatim(q: str):
    time.sleep(1.1)
    url = "https://nominatim.openstreetmap.org/search?" + urllib.parse.urlencode(
        {"q": q, "format": "json", "limit": 1}
    )
    req = urllib.request.Request(url, headers={"User-Agent": "sakaleyn-build/0.1 (district geocoding)"})
    try:
        with urllib.request.urlopen(req, timeout=30) as r:
            res = json.loads(r.read())
        return (float(res[0]["lat"]), float(res[0]["lon"])) if res else None
    except Exception as e:
        print(f"  nominatim hata: {q}: {e}", file=sys.stderr)
        return None


def km(a, b):
    p = math.pi / 180
    h = math.sin((b[0] - a[0]) * p / 2) ** 2 + math.cos(a[0] * p) * math.cos(b[0] * p) * math.sin(
        (b[1] - a[1]) * p / 2
    ) ** 2
    return 12742 * math.asin(math.sqrt(h))


def main(tree_p, en_p, geo_dir, out_p, use_nominatim, cache_p=None):
    tree = json.loads(Path(tree_p).read_text(encoding="utf-8"))
    en = {int(k): v for k, v in json.loads(Path(en_p).read_text(encoding="utf-8")).items()}
    geo = Path(geo_dir)

    # Ülke adı → ISO kodu (countryInfo.txt, İngilizce ad)
    iso = {}
    for line in (geo / "countryInfo.txt").read_text(encoding="utf-8").splitlines():
        if line.startswith("#") or not line.strip():
            continue
        c = line.split("\t")
        iso[norm(c[4])] = c[0]
    iso.update({norm(k): v for k, v in EXTRA_ISO.items()})

    # Türkiye
    tr_recs = list(read_geonames(geo / "TR.txt"))
    tr_adm1 = {norm(n): r["admin1"] for r in tr_recs if r["fcode"] == "ADM1" for n in r["names"]}
    tr_adm2 = index(r for r in tr_recs if r["fcode"] == "ADM2")
    tr_ppl = index(r for r in tr_recs if r["fcode"].startswith("PPL"))

    # Yurt dışı
    world_recs = list(read_geonames(geo / "cities500.txt"))
    world = index(r for r in world_recs if r["cc"] != "TR")
    tz_points = defaultdict(list)  # ülke → [(lat, lon, tz)] (saat dilimi için)
    tz_small = defaultdict(list)  # 5000'den küçük yerler (Vatikan, Svalbard gibi ülkeler için)
    for r in world_recs:
        if r["tz"]:
            (tz_points if r["pop"] >= 5000 else tz_small)[r["cc"]].append((r["lat"], r["lon"], r["tz"]))
    for cc, pts in tz_small.items():
        if not tz_points.get(cc):
            tz_points[cc] = pts
    adm1_names = {}
    for line in (geo / "admin1CodesASCII.txt").read_text(encoding="utf-8").splitlines():
        code, name, ascii_name, _ = line.split("\t")
        cc, a1 = code.split(".")
        adm1_names[(cc, norm(name))] = a1
        adm1_names[(cc, norm(ascii_name))] = a1

    out, missing = [], []
    for country in tree:
        if country["id"] in SKIP_COUNTRIES:
            continue
        en_name = en.get(country["id"], country["name"])
        cc = TR_COUNTRY_ISO.get(country["name"]) or iso.get(norm(en_name)) or iso.get(norm(country["name"]))
        for state in country["states"]:
            for d in state["districts"]:
                lat = lon = None
                src = None
                if cc == "TR":
                    a1 = tr_adm1.get(norm(state["name"]))
                    r = None
                    for n in variants(d["name"]):
                        if n == norm(state["name"]) or n in IL_MERKEZ:  # il merkezi
                            n = norm(IL_MERKEZ.get(n, n))
                            r = best([x for x in tr_ppl.get(n, []) if x["fcode"] in ("PPLA", "PPLC")], a1)
                        else:
                            r = best([x for x in tr_adm2.get(n, []) if x["admin1"] == a1]) or best(
                                [x for x in tr_ppl.get(n, []) if x["admin1"] == a1]
                            )
                        if r:
                            break
                    if r:
                        lat, lon, src = r["lat"], r["lon"], "geonames"
                elif cc:
                    a1 = adm1_names.get((cc, norm(state["name_en"] or state["name"])))
                    cands = []
                    for nm in {d["name"], d["name_en"]}:
                        for n in variants(nm):
                            cands += [x for x in world.get(n, []) if x["cc"] == cc]
                        if cands:
                            break
                    if a1:  # eyalet biliniyorsa başka eyaletteki aynı adlı şehir kabul edilmez
                        cands = [x for x in cands if x["admin1"] == a1]
                    r = best(cands, a1)
                    if r:
                        lat, lon, src = r["lat"], r["lon"], "geonames"
                entry = {
                    "id": d["id"],
                    "name": d["name"],
                    "state": state["name"],
                    "state_id": state["id"],
                    "country": country["name"],
                    "country_id": country["id"],
                    "cc": cc,
                    "lat": round(lat, 4) if lat is not None else None,
                    "lon": round(lon, 4) if lon is not None else None,
                    "src": src,
                }
                out.append(entry)
                if lat is None:
                    missing.append((entry, en_name, state.get("name_en") or state["name"], d["name_en"]))

    print(f"{len(out)} ilçe, GeoNames ile eşleşmeyen: {len(missing)}", file=sys.stderr)
    cache = {}
    if cache_p and Path(cache_p).exists():
        cache = json.loads(Path(cache_p).read_text(encoding="utf-8"))
    for i, (e, en_country, en_state, en_dist) in enumerate(missing):
        key = str(e["id"])
        if key not in cache:
            if not use_nominatim:
                continue
            q = f"{en_dist}, {en_state}, {en_country}" if en_state != en_country else f"{en_dist}, {en_country}"
            cache[key] = nominatim(q) or nominatim(f"{en_dist}, {en_country}")
            print(f"  [{i + 1}/{len(missing)}] {q}: {cache[key]}", file=sys.stderr)
            if cache_p and i % 20 == 0:  # yarıda kesilirse kaldığı yerden devam
                Path(cache_p).write_text(json.dumps(cache, ensure_ascii=False, indent=0), encoding="utf-8")
        if cache[key]:
            e["lat"], e["lon"], e["src"] = round(cache[key][0], 4), round(cache[key][1], 4), "osm"
    if cache_p:
        Path(cache_p).write_text(json.dumps(cache, ensure_ascii=False, indent=0), encoding="utf-8")

    for e in out:
        if e["cc"] == "TR":
            e["tz"] = "Europe/Istanbul"
            continue
        pts = tz_points.get(e["cc"], [])
        zones = {p[2] for p in pts}
        if len(zones) == 1:
            e["tz"] = zones.pop()
        elif pts and e["lat"] is not None:
            e["tz"] = min(pts, key=lambda p: km((e["lat"], e["lon"]), p[:2]))[2]
        else:
            e["tz"] = None
    # Koordinatı bulunamayan (çoğu yazım hatalı ad): aynı şehir/eyaletteki en yaygın saat dilimi.
    # Koordinat boş kalır: otomatik konumda seçilmez, elle seçilebilir.
    by_state = defaultdict(list)
    for e in out:
        if e["tz"]:
            by_state[(e["country_id"], e["state_id"])].append(e["tz"])
    for e in out:
        if not e["tz"] and by_state.get((e["country_id"], e["state_id"])):
            zs = by_state[(e["country_id"], e["state_id"])]
            e["tz"] = max(set(zs), key=zs.count)
    print(f"saat dilimi bulunamayan: {sum(e['tz'] is None for e in out)}", file=sys.stderr)

    still = [e for e in out if e["lat"] is None]
    print(f"koordinatsız kalan: {len(still)}", file=sys.stderr)
    for e in still:
        print(f"  {e['id']} {e['country']} / {e['state']} / {e['name']}", file=sys.stderr)
    for e in out:
        e.pop("src")
    Path(out_p).write_text(json.dumps(out, ensure_ascii=False, separators=(",", ":")), encoding="utf-8")


# Diyanet'in İngilizce ülke adı GeoNames'tekinden farklı olanlar (gerektikçe eklenir).
EXTRA_ISO = {
    "USA": "US",
    "TURKEY": "TR",
    "TURKIYE": "TR",
    "UNITED KINGDOM": "GB",
    "ENGLAND": "GB",
    "RUSSIA": "RU",
    "SOUTH KOREA": "KR",
    "NORTH KOREA": "KP",
    "IRAN": "IR",
    "SYRIA": "SY",
    "BOSNIA AND HERZEGOVINA": "BA",
    "CZECH REPUBLIC": "CZ",
    "NETHERLANDS": "NL",
    "TRNC": "CY",
    "NORTHERN CYPRUS": "CY",
}

# Okyanus ızgarası (gemiler için "ATLAS(enlem+boylam)" noktaları): masaüstünde anlamsız.
SKIP_COUNTRIES = {1216}

# Türkiye: Diyanet adı → GeoNames adı
ALIASES = {
    "mereglisi": "Marmaraereğlisi",
    "karadenizeregli": "Ereğli",
}
# İl adıyla aynı olmayan il merkezleri (Diyanet ilçe adı → merkez şehir)
IL_MERKEZ = {"kocaeli": "İzmit", "sakarya": "Adapazarı", "hatay": "Antakya"}

# Diyanet'in Türkçe ülke adı → ISO (İngilizce adı GeoNames'le eşleşmeyenler)
TR_COUNTRY_ISO = {
    "ANTIGUA VE BARBUDA": "AG", "ASCENSION": "SH", "BOLIVYA": "BO", "BOSNA HERSEK": "BA",
    "BUTAN": "BT", "CAD": "TD", "CIBUTI": "DJ", "DEMOKRATIK KONGO CUMHURIYETI": "CD",
    "DOGU TIMOR": "TL", "DOMINIK": "DM", "DOMINIK CUMHURIYETI": "DO", "EKVATOR": "EC",
    "EKVATOR GINESI": "GQ", "ERITRE": "ER", "ESTONYA": "EE", "FILDISI SAHILI": "CI",
    "FILISTIN": "PS", "GAMBIYA": "GM", "GRONLAND": "GL", "GUADELOPE": "GP",
    "GUAM ADASI": "GU", "IZLANDA": "IS", "JAMAIKA": "JM", "KAMBOCYA": "KH",
    "KARADAG": "ME", "KATAR": "QA", "KIRGIZISTAN": "KG", "KOLOMBIYA": "CO",
    "KOMORLAR": "KM", "KOSOVA": "XK", "KOSTARIKA": "CR", "KUDUS": "PS",
    "KUZEY KIBRIS": "CY", "LESOTO": "LS", "LIBERYA": "LR", "MADAGASKAR": "MG",
    "MAKAO": "MO", "MAKEDONYA": "MK", "MALAVI": "MW", "MALDIVLER": "MV",
    "MAN ADASI": "IM", "MARTINIK": "MQ", "MAURITIUS ADASI": "MU", "MIKRONEZYA": "FM",
    "MOLDAVYA": "MD", "MONTSERRAT (U.K.)": "MS", "MORITANYA": "MR", "MOZAMBIK": "MZ",
    "NAMIBYA": "NA", "NIJER": "NE", "NIKARAGUA": "NI", "ORTA AFRIKA CUMHURIYETI": "CF",
    "PAPUA YENI GINE": "PG", "PITCAIRN ADASI": "PN", "RUANDA": "RW", "SEYSEL ADALARI": "SC",
    "SIRBISTAN": "RS", "ST. LUCIA": "LC", "SURINAM": "SR", "SVALBARD": "SJ",
    "TANZANYA": "TZ", "TRINIDAT VE TOBAGO": "TT", "TUNUS": "TN", "UKRAYNA-KIRIM": "UA",
    "UMMAN": "OM", "VATIKAN": "VA", "YENI KALEDONYA": "NC", "YESIL BURUN": "CV",
    "ZAMBIYA": "ZM", "ZIMBABVE": "ZW",
}

if __name__ == "__main__":
    argv = sys.argv[1:]
    cache_p = None
    if "--cache" in argv:
        i = argv.index("--cache")
        cache_p = argv[i + 1]
        del argv[i : i + 2]
    a = [x for x in argv if not x.startswith("--")]
    main(*a, use_nominatim="--nominatim" in argv, cache_p=cache_p)

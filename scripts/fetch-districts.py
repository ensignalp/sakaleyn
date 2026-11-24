#!/usr/bin/env python3
"""Diyanet ülke → şehir → ilçe ağacını indirir.

Kullanım: python3 -I scripts/fetch-districts.py <çıktı.json>
Kaynak: namazvakitleri.diyanet.gov.tr (sitenin kendi açılır listeleri).
İstekler arasında bekler; bir kez çalıştırılıp sonuç depoya konur.
"""

import html
import json
import re
import sys
import time
import urllib.parse
import urllib.request

BASE = "https://namazvakitleri.diyanet.gov.tr"
UA = "Mozilla/5.0 (X11; Linux x86_64) sakaleyn-build"
DELAY = 0.35


def get(url: str) -> str:
    for attempt in range(4):
        try:
            req = urllib.request.Request(
                url, headers={"User-Agent": UA, "X-Requested-With": "XMLHttpRequest"}
            )
            with urllib.request.urlopen(req, timeout=30) as r:
                return r.read().decode("utf-8")
        except Exception as e:  # ağ hatası: kısa bekleyip tekrar dene
            print(f"  tekrar ({attempt + 1}): {url}: {e}", file=sys.stderr)
            time.sleep(2 * (attempt + 1))
    raise RuntimeError(f"alınamadı: {url}")


def reg_list(**params) -> dict:
    params["Culture"] = "tr-TR"
    time.sleep(DELAY)
    url = f"{BASE}/tr-TR/home/GetRegList?{urllib.parse.urlencode(params)}"
    return json.loads(get(url))


def districts_of(lst) -> list:
    return [
        {"id": int(d["IlceID"]), "name": d["IlceAdi"], "name_en": d["IlceAdiEn"]}
        for d in lst or []
    ]


def main(out_path: str) -> None:
    home = get(f"{BASE}/tr-TR")
    sel = re.search(r'<select class="country-select.*?</select>', home, re.S).group(0)
    countries = [
        (int(v), html.unescape(n).strip())
        for v, n in re.findall(r'<option[^>]*value="(\d+)"[^>]*>([^<]*)', sel)
    ]
    print(f"{len(countries)} ülke", file=sys.stderr)

    result = []
    for cid, cname in countries:
        data = reg_list(ChangeType="country", CountryId=cid)
        country = {"id": cid, "name": cname, "states": []}
        if data.get("StateList"):
            for s in data["StateList"]:
                sid = int(s["SehirID"])
                sd = reg_list(ChangeType="state", CountryId=cid, StateId=sid)
                country["states"].append(
                    {
                        "id": sid,
                        "name": s["SehirAdi"],
                        "name_en": s["SehirAdiEn"],
                        "districts": districts_of(sd.get("StateRegionList")),
                    }
                )
        else:  # şehir listesi olmayan ülke: ilçeler doğrudan gelir
            country["states"].append(
                {
                    "id": None,
                    "name": cname,
                    "name_en": cname,
                    "districts": districts_of(data.get("StateRegionList")),
                }
            )
        n = sum(len(s["districts"]) for s in country["states"])
        print(f"{cname}: {len(country['states'])} şehir, {n} ilçe", file=sys.stderr)
        result.append(country)

    with open(out_path, "w", encoding="utf-8") as f:
        json.dump(result, f, ensure_ascii=False, indent=1)


if __name__ == "__main__":
    main(sys.argv[1])

//! Gömülü Diyanet ilçe veritabanı (`assets/districts.json`).
//!
//! `scripts/build-districts.py` üretir: Diyanet ilçe kimliği, ad, şehir/il, ülke,
//! koordinat (GeoNames / OSM) ve IANA saat dilimi. Koordinatı olmayan kayıtlar
//! otomatik konumda seçilmez, ama aranıp elle seçilebilir.
//!
//! Diyanet her ilçe için tablo yayınlamaz (İstanbul'da 19 ilçe var; Üsküdar,
//! Kadıköy, Beylerbeyi "İSTANBUL" tablosunu kullanır). `assets/places.json`
//! (`scripts/build-places.py`, GeoNames) Türkiye'deki ilçe, semt ve köyleri bu
//! kurala göre bir Diyanet ilçesine bağlar; arama ve otomatik konum onu da kullanır.

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::text::{fold, title};

static RAW: &str = include_str!("../../../assets/districts.json");
static DB: OnceLock<Vec<District>> = OnceLock::new();
static PLACES_RAW: &str = include_str!("../../../assets/places.json");
static PLACES: OnceLock<Places> = OnceLock::new();

/// Türkiye'de bir yer (ilçe, semt, köy) ve tablosunu kullandığı Diyanet ilçesi.
#[derive(Clone, Debug)]
pub struct Place {
    pub name: String,
    folded: String,
    il: usize,
    pub lat: f64,
    pub lon: f64,
    /// 0 ilçe, 1 semt/merkez, 2 köy/kasaba.
    pub kind: u8,
    pub population: u64,
    pub district: u32,
}

struct Places {
    il: Vec<String>,
    folded_il: Vec<String>,
    items: Vec<Place>,
}

#[derive(Deserialize)]
struct PlacesFile {
    il: Vec<String>,
    p: Vec<(String, usize, f64, f64, u8, u64, u32)>,
}

fn places() -> &'static Places {
    PLACES.get_or_init(|| {
        let f: PlacesFile = serde_json::from_str(PLACES_RAW).expect("assets/places.json is valid");
        Places {
            folded_il: f.il.iter().map(|i| fold(i)).collect(),
            items: f
                .p
                .into_iter()
                .map(|(name, il, lat, lon, kind, population, district)| Place {
                    folded: fold(&name),
                    name,
                    il,
                    lat,
                    lon,
                    kind,
                    population,
                    district,
                })
                .collect(),
            il: f.il,
        }
    })
}

impl Place {
    /// "Üsküdar, İstanbul"
    pub fn display(&self) -> String {
        format!("{}, {}", self.name, title(&places().il[self.il], true))
    }
}

/// Arama sonucu: seçilince kullanılacak Diyanet ilçesi ve gösterilecek ad.
#[derive(Clone, Debug)]
pub struct Hit {
    pub district: &'static District,
    /// "Üsküdar, İstanbul" ya da Diyanet ilçesinin kendi adı.
    pub name: String,
    /// Yer, adı farklı bir Diyanet ilçesinin tablosunu kullanıyorsa o ilçe ("İstanbul").
    pub via: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct District {
    pub id: u32,
    pub name: String,
    pub state: String,
    pub state_id: Option<u32>,
    pub country: String,
    pub country_id: u32,
    pub cc: Option<String>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub tz: Option<String>,
}

impl District {
    pub fn is_turkey(&self) -> bool {
        self.cc.as_deref() == Some("TR")
    }

    /// Ekranda gösterilecek ad: `"Kadıköy, İstanbul"`, `"Berlin, Almanya"`.
    /// İl merkezi için yalnızca il adı (`"İstanbul"`).
    pub fn display(&self) -> String {
        let tr = self.is_turkey();
        let name = title(&self.name, tr);
        if self.is_turkey() {
            if fold(&self.name) == fold(&self.state) {
                name
            } else {
                format!("{name}, {}", title(&self.state, true))
            }
        } else {
            let country = self.country_name();
            if fold(&self.state) == fold(&self.country) || fold(&self.state) == fold(&self.name) {
                format!("{name}, {country}")
            } else {
                format!("{name}, {}, {country}", title(&self.state, false))
            }
        }
    }

    /// Ülke adı başlık biçiminde; kısaltmalar (ABD, KKTC, BAE) olduğu gibi.
    pub fn country_name(&self) -> String {
        match self.country.as_str() {
            "ABD" | "KKTC" | "BAE" => self.country.clone(),
            c => title(c, true),
        }
    }

    /// Saat dilimi; bilinmiyorsa Türkiye saati.
    pub fn tz(&self) -> chrono_tz::Tz {
        self.tz
            .as_deref()
            .and_then(|z| z.parse().ok())
            .unwrap_or(chrono_tz::Europe::Istanbul)
    }

    pub fn coords(&self) -> Option<(f64, f64)> {
        Some((self.lat?, self.lon?))
    }
}

pub fn all() -> &'static [District] {
    DB.get_or_init(|| serde_json::from_str(RAW).expect("assets/districts.json is valid"))
}

pub fn by_id(id: u32) -> Option<&'static District> {
    all().iter().find(|d| d.id == id)
}

/// Büyük daire mesafesi (km).
pub fn km(a: (f64, f64), b: (f64, f64)) -> f64 {
    let p = std::f64::consts::PI / 180.0;
    let h = ((b.0 - a.0) * p / 2.0).sin().powi(2)
        + (a.0 * p).cos() * (b.0 * p).cos() * ((b.1 - a.1) * p / 2.0).sin().powi(2);
    12742.0 * h.sqrt().asin()
}

/// Koordinata en yakın ilçe ve uzaklığı (km).
pub fn nearest(lat: f64, lon: f64) -> Option<(&'static District, f64)> {
    all()
        .iter()
        .filter_map(|d| d.coords().map(|c| (d, km((lat, lon), c))))
        .min_by(|a, b| a.1.total_cmp(&b.1))
}

/// Koordinattaki yerin Diyanet ilçesi: Türkiye içinde en yakın semt/köy/ilçenin
/// bağlı olduğu ilçe (Diyanet kuralı), dışında en yakın Diyanet şehri.
/// Dönüş: (ilçe, en yakın kayda uzaklık km, Türkiye'de yerin adı).
pub fn locate(lat: f64, lon: f64) -> Option<(&'static District, f64, Option<String>)> {
    let in_turkey = (35.5..42.5).contains(&lat) && (25.5..45.0).contains(&lon);
    if in_turkey
        && let Some((p, d)) = places()
            .items
            .iter()
            .map(|p| (p, km((lat, lon), (p.lat, p.lon))))
            .min_by(|a, b| a.1.total_cmp(&b.1))
        && d <= 25.0
        && let Some(district) = by_id(p.district)
    {
        return Some((district, d, Some(p.display())));
    }
    nearest(lat, lon).map(|(d, km)| (d, km, None))
}

/// Ada göre arama: Diyanet ilçeleri ve Türkiye'deki ilçe/semt/köyler. Sorgu
/// Türkçe karakter ve büyük/küçük harften bağımsızdır (`"USKUDAR"` = `"Üsküdar"`).
/// Birden çok kelime varsa hepsi ad/il/ülke içinde geçmelidir (`"kadıköy ist"`).
/// Sıralama: tam ad, ad başı, içinde geçen; Diyanet ilçesi ve ilçe düzeyi önce.
pub fn search(query: &str, limit: usize) -> Vec<Hit> {
    let words: Vec<String> = query.split_whitespace().map(fold).filter(|w| !w.is_empty()).collect();
    if words.is_empty() {
        return Vec::new();
    }
    let rank_of = |name: &str| {
        let first = &words[0];
        if name == first {
            0u8
        } else if name.starts_with(first.as_str()) {
            2
        } else {
            4
        }
    };
    // (sıra, nüfus, sonuç)
    let mut hits: Vec<(u8, u64, Hit)> = search_districts(&words)
        .into_iter()
        .map(|(r, d)| (r * 2, u64::MAX, Hit { district: d, name: d.display(), via: None }))
        .collect();
    let ps = places();
    for p in &ps.items {
        let hay = format!("{} {}", p.folded, ps.folded_il[p.il]);
        if !words.iter().all(|w| hay.contains(w.as_str())) {
            continue;
        }
        let Some(d) = by_id(p.district) else { continue };
        if fold(&d.name) == p.folded {
            continue; // Diyanet ilçesinin kendisi; zaten listede
        }
        let via = Some(d.display());
        hits.push((rank_of(&p.folded) * 2 + 1 + p.kind, p.population, Hit { district: d, name: p.display(), via }));
    }
    hits.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| b.1.cmp(&a.1)));
    let mut seen = std::collections::HashSet::new();
    hits.into_iter()
        .filter(|(_, _, h)| seen.insert(fold(&h.name)))
        .take(limit)
        .map(|(_, _, h)| h)
        .collect()
}

fn search_districts(words: &[String]) -> Vec<(u8, &'static District)> {
    let mut hits: Vec<(u8, &District)> = all()
        .iter()
        .filter_map(|d| {
            let name = fold(&d.name);
            let hay = format!("{name} {} {}", fold(&d.state), fold(&d.country));
            if !words.iter().all(|w| hay.contains(w.as_str())) {
                return None;
            }
            let first = &words[0];
            let mut rank = if &name == first {
                0
            } else if name.starts_with(first.as_str()) {
                2
            } else {
                4
            };
            if !d.is_turkey() {
                rank += 1;
            }
            // il merkezi ilçesi ("İSTANBUL / İSTANBUL") aynı ada sahip ilçelerin önünde
            if fold(&d.state) == name && rank > 0 {
                rank -= 1;
            }
            Some((rank, d))
        })
        .collect();
    hits.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.name.len().cmp(&b.1.name.len())));
    hits
}

/// GeoClue yoksa ilk açılış için tahmin: sistem saat dilimiyle aynı dilimdeki
/// bir il merkezi (Türkiye için İstanbul).
pub fn guess_from_timezone(tz: &str) -> Option<&'static District> {
    if tz == "Europe/Istanbul" || tz == "Asia/Istanbul" || tz == "Turkey" {
        return by_id(9541);
    }
    all()
        .iter()
        .filter(|d| d.tz.as_deref() == Some(tz) && d.coords().is_some())
        .find(|d| fold(&d.name) == fold(&d.state))
        .or_else(|| all().iter().find(|d| d.tz.as_deref() == Some(tz) && d.coords().is_some()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_and_finds() {
        assert!(all().len() > 8000);
        let ist = by_id(9541).unwrap();
        assert_eq!(ist.display(), "İstanbul");
        assert_eq!(ist.tz(), chrono_tz::Europe::Istanbul);
        assert_eq!(by_id(11002).unwrap().display(), "Berlin, Almanya");
        assert_eq!(by_id(8903).unwrap().display(), "New York, ABD");
        assert_eq!(by_id(11002).unwrap().tz(), chrono_tz::Europe::Berlin);
    }

    #[test]
    fn every_turkish_district_has_coords_inside_turkey() {
        let tr: Vec<_> = all().iter().filter(|d| d.is_turkey()).collect();
        assert!(tr.len() > 850);
        for d in tr {
            let (lat, lon) = d.coords().unwrap_or_else(|| panic!("{} has no coords", d.name));
            assert!((35.5..42.5).contains(&lat) && (25.5..45.0).contains(&lon), "{}: {lat},{lon}", d.name);
        }
    }

    #[test]
    fn nearest_district() {
        // Kadıköy iskelesi
        let (d, km) = nearest(40.990, 29.025).unwrap();
        assert_eq!(fold(&d.state), "istanbul");
        assert!(km < 10.0, "{} {km}", d.name);
        // Van merkez
        let (d, _) = nearest(38.50, 43.38).unwrap();
        assert_eq!(d.id, 9930);
    }

    #[test]
    fn search_ranks() {
        let r = search("pendik", 5);
        assert_eq!(fold(&r[0].district.name), "pendik");
        assert_eq!(r[0].name, "Pendik, İstanbul");
        let r = search("ankara", 3);
        assert_eq!(r[0].district.id, 9206);
        let r = search("berlin", 3);
        assert_eq!(r[0].district.id, 11002);
        assert!(search("", 3).is_empty());
        let r = search("merkez van", 5);
        assert!(r.is_empty() || r.iter().all(|h| fold(&h.district.state) == "van"));
    }

    /// Diyanet listesinde olmayan ilçe ve semtler il merkezinin tablosuna bağlanır;
    /// Türkçe karakter ve büyük/küçük harf farkı önemsizdir.
    #[test]
    fn search_places_and_normalization() {
        for q in ["üsküdar", "Üsküdar", "ÜSKÜDAR", "uskudar", "USKUDAR"] {
            let r = search(q, 5);
            assert_eq!(r[0].name, "Üsküdar, İstanbul", "{q}");
            assert_eq!(r[0].district.id, 9541, "{q}");
            assert_eq!(r[0].via.as_deref(), Some("İstanbul"));
        }
        for q in ["beylerbeyi", "Beylerbeyi", "BEYLERBEYİ", "beylerbeyi istanbul"] {
            let r = search(q, 5);
            let h = r.iter().find(|h| h.name == "Beylerbeyi, İstanbul").unwrap_or_else(|| panic!("{q}: {r:?}"));
            assert_eq!(h.district.id, 9541);
        }
        // İ/ı: "ISPARTA" (Diyanet) ve "ısparta" aynı
        assert_eq!(search("ısparta", 1)[0].district.name, "ISPARTA");
        assert_eq!(search("İSPARTA", 1)[0].district.name, "ISPARTA");
        assert_eq!(search("isparta", 10).iter().filter(|h| h.district.name == "ISPARTA" && h.via.is_none()).count(), 1);
        assert!(!search("isparta", 10).iter().any(|h| h.name == "Isparta, Isparta"));
        // Kendi tablosu olan ilçe kendi kimliğiyle gelir, tekrar etmez
        let r = search("kartal", 10);
        assert_eq!(r[0].district.id, 9542);
        assert_eq!(r.iter().filter(|h| h.name == "Kartal, İstanbul").count(), 1);
        // Van'ın merkez ilçeleri Van tablosunu kullanır
        assert_eq!(search("tuşba", 1)[0].district.id, 9930);
    }

    #[test]
    fn locate_maps_neighbourhoods() {
        // Beylerbeyi sarayı civarı
        let (d, km, place) = locate(41.043, 29.046).unwrap();
        assert_eq!(d.id, 9541);
        assert!(km < 2.0, "{km}");
        assert!(place.unwrap().ends_with("İstanbul"));
        // Pendik merkez kendi tablosunu kullanır
        assert_eq!(locate(40.877, 29.234).unwrap().0.id, 9545);
        // Berlin: Türkiye dışı, en yakın Diyanet şehri
        let (d, _, place) = locate(52.52, 13.40).unwrap();
        assert_eq!(d.id, 11002);
        assert!(place.is_none());
    }

    #[test]
    fn timezone_guess() {
        assert_eq!(guess_from_timezone("Europe/Istanbul").unwrap().id, 9541);
        assert_eq!(guess_from_timezone("Europe/Berlin").unwrap().tz(), chrono_tz::Europe::Berlin);
    }
}

//! namazvakitleri.diyanet.gov.tr ilçe sayfasının ayrıştırılması.
//!
//! İlçe sayfasında (`/tr-TR/<ilçe>`) üç tablo vardır: `#tab-0` haftalık,
//! `#tab-1` bugünden itibaren 30 gün, `#tab-2` takvim yılının tamamı. Sütunlar:
//! Miladi, Hicri, İmsak, Güneş, Öğle, İkindi, Akşam, Yatsı (`HH:MM`).
//! Yıllık ve 30 günlük tablo birleştirilir; böylece Aralık sonunda bile
//! Ocak'ın ilk haftaları elde olur. Ayrıştırıcı HTML kütüphanesi kullanmaz:
//! yalnızca `<tr>`/`<td>` yapısına bakar, sınıf adlarına ve düzene bağlı değildir.

use std::collections::BTreeMap;

use chrono::{NaiveDate, NaiveTime};

use crate::http::{FetchError, get_text};
use crate::model::DayTimes;

pub const BASE: &str = "https://namazvakitleri.diyanet.gov.tr";

pub fn district_url(id: u32) -> String {
    format!("{BASE}/tr-TR/{id}")
}

/// İlçenin vakit tablosunu indirir (yıl + önümüzdeki 30 gün).
pub async fn fetch_district(client: &reqwest::Client, id: u32) -> Result<Vec<DayTimes>, FetchError> {
    let html = get_text(client, &district_url(id)).await?;
    let days = parse_page(&html);
    if days.len() < 7 {
        return Err(FetchError::Parse(format!("only {} days found", days.len())));
    }
    Ok(days)
}

/// Sayfadaki bütün vakit satırlarını tarihe göre birleştirip sıralı döndürür.
pub fn parse_page(html: &str) -> Vec<DayTimes> {
    let mut by_date = BTreeMap::new();
    for row in rows(html) {
        if let Some(d) = parse_row(&row) {
            by_date.insert(d.date, d);
        }
    }
    by_date.into_values().collect()
}

/// `<tr>…</tr>` içindeki hücre metinleri.
fn rows(html: &str) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    let lower = html.to_ascii_lowercase();
    let mut pos = 0;
    while let Some(start) = lower[pos..].find("<tr") {
        let start = pos + start;
        let Some(end) = lower[start..].find("</tr>") else { break };
        let end = start + end;
        out.push(cells(&html[start..end], &lower[start..end]));
        pos = end + 5;
    }
    out
}

fn cells(row: &str, lower: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(i) = lower[pos..].find("<td") {
        let open = pos + i;
        let Some(gt) = lower[open..].find('>') else { break };
        let content_start = open + gt + 1;
        let content_end = lower[content_start..]
            .find("</td>")
            .map(|e| content_start + e)
            .unwrap_or(row.len());
        out.push(clean(&row[content_start..content_end]));
        pos = content_end;
    }
    out
}

/// Etiketleri atar, HTML varlıklarını çözer, boşlukları sadeleştirir.
fn clean(s: &str) -> String {
    let mut text = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => text.push(c),
            _ => {}
        }
    }
    let text = unescape(&text);
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let Some(semi) = rest[..rest.len().min(12)].find(';') else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let ent = &rest[1..semi];
        let ch = match ent {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            e if e.starts_with("#x") || e.starts_with("#X") => {
                u32::from_str_radix(&e[2..], 16).ok().and_then(char::from_u32)
            }
            e if e.starts_with('#') => e[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match ch {
            Some(c) => {
                out.push(c);
                rest = &rest[semi + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

const MONTHS: [&str; 12] = [
    "ocak", "subat", "mart", "nisan", "mayis", "haziran", "temmuz", "agustos", "eylul", "ekim", "kasim", "aralik",
];

/// `"23 Kasım 2026 Pazartesi"` → 2026-11-23
pub fn parse_tr_date(s: &str) -> Option<NaiveDate> {
    let mut it = s.split_whitespace();
    let day: u32 = it.next()?.parse().ok()?;
    let month = crate::text::fold(it.next()?);
    let year: i32 = it.next()?.parse().ok()?;
    let m = MONTHS.iter().position(|x| *x == month)? as u32 + 1;
    NaiveDate::from_ymd_opt(year, m, day)
}

fn parse_row(c: &[String]) -> Option<DayTimes> {
    if c.len() < 8 {
        return None;
    }
    let date = parse_tr_date(&c[0])?;
    let mut times = [NaiveTime::MIN; 6];
    for (i, t) in c[2..8].iter().enumerate() {
        times[i] = NaiveTime::parse_from_str(t.trim(), "%H:%M").ok()?;
    }
    Some(DayTimes {
        date,
        hijri: c[1].clone(),
        times,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Vakit;

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    fn t(s: &str) -> NaiveTime {
        NaiveTime::parse_from_str(s, "%H:%M").unwrap()
    }

    #[test]
    fn parses_istanbul_year_plus_month() {
        let days = parse_page(&fixture("diyanet-9541-istanbul-2026-11-23.html"));
        // 365 gün (2026) + 30 günlük tablodan Ocak 2027'ye taşan günler yok (23 Kasım'dan 30 gün)
        assert_eq!(days.first().unwrap().date, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        assert_eq!(days.last().unwrap().date, NaiveDate::from_ymd_opt(2026, 12, 31).unwrap());
        assert_eq!(days.len(), 365);
        let today = days
            .iter()
            .find(|d| d.date == NaiveDate::from_ymd_opt(2026, 11, 23).unwrap())
            .unwrap();
        assert_eq!(today.hijri, "14 Cemaziyelahir 1448");
        assert_eq!(today.time(Vakit::Imsak), t("06:24"));
        assert_eq!(today.time(Vakit::Gunes), t("07:53"));
        assert_eq!(today.time(Vakit::Ogle), t("12:55"));
        assert_eq!(today.time(Vakit::Ikindi), t("15:25"));
        assert_eq!(today.time(Vakit::Aksam), t("17:47"));
        assert_eq!(today.time(Vakit::Yatsi), t("19:11"));
    }

    #[test]
    fn parses_all_fixtures() {
        for (f, last_yatsi) in [
            ("diyanet-9206-ankara-2026-11-23.html", "19:05"),
            ("diyanet-9419-edirne-2026-11-23.html", "19:27"),
            ("diyanet-9930-van-2026-11-23.html", "18:25"),
            ("diyanet-11002-berlin-2026-11-23.html", "17:55"),
        ] {
            let days = parse_page(&fixture(f));
            assert_eq!(days.len(), 365, "{f}");
            assert_eq!(days.last().unwrap().time(Vakit::Yatsi), t(last_yatsi), "{f}");
            for w in days.windows(2) {
                assert_eq!(w[0].date.succ_opt().unwrap(), w[1].date, "{f}: gap");
            }
            for d in &days {
                assert!(d.times.windows(2).all(|p| p[0] < p[1]), "{f}: {} not increasing", d.date);
            }
        }
    }

    #[test]
    fn ramadan_from_hijri() {
        let days = parse_page(&fixture("diyanet-9541-istanbul-2026-11-23.html"));
        let r: Vec<_> = days.iter().filter(|d| d.is_ramadan()).collect();
        assert!((29..=30).contains(&r.len()), "{}", r.len());
        // 2026 Ramazanı Şubat-Mart'ta
        assert_eq!(r[0].date.format("%Y-%m").to_string(), "2026-02");
    }

    #[test]
    fn dates_and_entities() {
        assert_eq!(parse_tr_date("01 Ocak 2026 Perşembe"), NaiveDate::from_ymd_opt(2026, 1, 1));
        assert_eq!(parse_tr_date("29 Şubat 2028 Salı"), NaiveDate::from_ymd_opt(2028, 2, 29));
        assert_eq!(parse_tr_date("31 Ağustos 2026"), NaiveDate::from_ymd_opt(2026, 8, 31));
        assert_eq!(parse_tr_date("x"), None);
        assert_eq!(unescape("bug&#252;nk&#252; &amp; &#x131;"), "bugünkü & ı");
        assert_eq!(clean("<b> 06:24 </b>\n"), "06:24");
    }
}

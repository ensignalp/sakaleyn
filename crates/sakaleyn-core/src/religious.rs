//! Diyanet dini günler listesi: `vakithesaplama.diyanet.gov.tr/dinigunler.php?yil=<yıl>`.
//!
//! Tablo satırları: hicri gün, hicri ay, hicri yıl, miladi gün, `"OCAK -2026"`,
//! haftanın günü, dini gün adı. Adı `"....."` olan satırlar (ay başları) atlanır.
//! Önbellek: `~/.cache/sakaleyn/religious-<yıl>.json` (yılda bir istek).

use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::http::{FetchError, get_text};
use crate::text::{fold, title};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReligiousDay {
    pub date: NaiveDate,
    /// Başlık biçiminde ad, ör. `"Mirac Kandili"`.
    pub name: String,
    pub hijri: String,
}

impl ReligiousDay {
    /// Kandiller akşamdan başlar ("bu gece"), diğerleri gündüz ("bugün").
    pub fn is_night(&self) -> bool {
        let f = fold(&self.name);
        f.contains("kandil") || f.contains("kadir")
    }
}

pub fn url(year: i32) -> String {
    format!("https://vakithesaplama.diyanet.gov.tr/dinigunler.php?yil={year}")
}

pub fn cache_path(dir: &Path, year: i32) -> PathBuf {
    dir.join(format!("religious-{year}.json"))
}

pub fn load(dir: &Path, year: i32) -> Option<Vec<ReligiousDay>> {
    serde_json::from_slice(&std::fs::read(cache_path(dir, year)).ok()?).ok()
}

pub fn save(dir: &Path, year: i32, days: &[ReligiousDay]) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(cache_path(dir, year), serde_json::to_vec(days).map_err(std::io::Error::other)?)
}

pub async fn fetch(client: &reqwest::Client, year: i32) -> Result<Vec<ReligiousDay>, FetchError> {
    let html = get_text(client, &url(year)).await?;
    let days = parse(&html);
    if days.is_empty() {
        return Err(FetchError::Parse("no religious days found".into()));
    }
    Ok(days)
}

const MONTHS: [&str; 12] = [
    "ocak", "subat", "mart", "nisan", "mayis", "haziran", "temmuz", "agustos", "eylul", "ekim", "kasim", "aralik",
];

pub fn parse(html: &str) -> Vec<ReligiousDay> {
    let mut out = Vec::new();
    let lower = html.to_ascii_lowercase();
    let mut pos = 0;
    while let Some(s) = lower[pos..].find("<tr") {
        let start = pos + s;
        let Some(e) = lower[start..].find("</tr>") else { break };
        let end = start + e;
        let cells = cells(&html[start..end]);
        pos = end + 5;
        if cells.len() < 7 {
            continue;
        }
        let name = cells[6].trim();
        if name.is_empty() || name.starts_with('.') {
            continue;
        }
        let Ok(day) = cells[3].trim().parse::<u32>() else { continue };
        // "OCAK -2026"
        let my = cells[4].replace(' ', "");
        let mut parts = my.split('-');
        let (Some(m), Some(y)) = (parts.next(), parts.next()) else { continue };
        let Some(mi) = MONTHS.iter().position(|x| *x == fold(m)) else { continue };
        let Ok(year) = y.parse::<i32>() else { continue };
        let Some(date) = NaiveDate::from_ymd_opt(year, mi as u32 + 1, day) else { continue };
        out.push(ReligiousDay {
            date,
            name: title(name, true),
            hijri: format!("{} {} {}", cells[0].trim(), title(cells[1].trim(), true), cells[2].trim()),
        });
    }
    out
}

fn cells(row: &str) -> Vec<String> {
    let mut out = Vec::new();
    let lower = row.to_ascii_lowercase();
    let mut pos = 0;
    while let Some(i) = lower[pos..].find("<td") {
        let open = pos + i;
        let Some(gt) = lower[open..].find('>') else { break };
        let cs = open + gt + 1;
        let ce = lower[cs..].find("</td>").map(|e| cs + e).unwrap_or(row.len());
        let mut text = String::new();
        let mut tag = false;
        for c in row[cs..ce].chars() {
            match c {
                '<' => tag = true,
                '>' => tag = false,
                c if !tag => text.push(c),
                _ => {}
            }
        }
        out.push(text.replace("&nbsp;", " ").split_whitespace().collect::<Vec<_>>().join(" "));
        pos = ce;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_2026() {
        let html = std::fs::read(format!("{}/tests/fixtures/dinigunler-2026.html", env!("CARGO_MANIFEST_DIR"))).unwrap();
        let html = String::from_utf8_lossy(&html);
        let days = parse(&html);
        assert!(days.len() >= 10, "{days:?}");
        let mirac = &days[0];
        assert_eq!(mirac.date, NaiveDate::from_ymd_opt(2026, 1, 15).unwrap());
        assert_eq!(fold(&mirac.name), "mirackandili");
        assert!(mirac.is_night());
        assert!(days.iter().any(|d| fold(&d.name).contains("ramazanbayrami")));
        assert!(days.iter().all(|d| d.date.format("%Y").to_string() == "2026"));
    }
}

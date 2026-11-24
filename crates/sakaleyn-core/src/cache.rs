//! İlçe vakit tablosu önbelleği: `~/.cache/sakaleyn/times/<ilçe>.json`.
//!
//! Diyanet sayfası yılın tamamını + önümüzdeki 30 günü verir; ilçe başına
//! ayda bir istek yeter. Yenileme kuralı: elde 14 günden az veri kaldıysa ya da
//! son indirme 30 günden eskiyse (Diyanet düzeltmeleri için).

use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use crate::model::DayTimes;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimesCache {
    pub district_id: u32,
    pub fetched_at: DateTime<Utc>,
    pub days: Vec<DayTimes>,
}

pub fn path_for(dir: &Path, id: u32) -> PathBuf {
    dir.join("times").join(format!("{id}.json"))
}

impl TimesCache {
    pub fn new(district_id: u32) -> Self {
        TimesCache {
            district_id,
            fetched_at: DateTime::<Utc>::MIN_UTC,
            days: Vec::new(),
        }
    }

    pub fn load(dir: &Path, id: u32) -> Option<TimesCache> {
        let bytes = std::fs::read(path_for(dir, id)).ok()?;
        serde_json::from_slice::<TimesCache>(&bytes)
            .ok()
            .filter(|c| c.district_id == id)
    }

    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        let p = path_for(dir, self.district_id);
        std::fs::create_dir_all(p.parent().unwrap())?;
        let tmp = p.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec(self).map_err(std::io::Error::other)?)?;
        std::fs::rename(tmp, p)
    }

    /// Yeni günleri ekler (aynı tarih varsa yenisi geçerli), 40 günden eski günleri atar.
    pub fn merge(&mut self, new: Vec<DayTimes>, now: DateTime<Utc>, today: NaiveDate) {
        let cutoff = today - Duration::days(40);
        let mut map: std::collections::BTreeMap<NaiveDate, DayTimes> =
            self.days.drain(..).map(|d| (d.date, d)).collect();
        for d in new {
            map.insert(d.date, d);
        }
        self.days = map.into_values().filter(|d| d.date >= cutoff).collect();
        self.fetched_at = now;
    }

    pub fn get(&self, date: NaiveDate) -> Option<&DayTimes> {
        self.days
            .binary_search_by(|d| d.date.cmp(&date))
            .ok()
            .map(|i| &self.days[i])
    }

    pub fn covered_until(&self) -> Option<NaiveDate> {
        self.days.last().map(|d| d.date)
    }

    pub fn needs_refresh(&self, today: NaiveDate, now: DateTime<Utc>) -> bool {
        let short = self.covered_until().is_none_or(|end| end < today + Duration::days(14));
        let missing_today = self.get(today).is_none();
        let stale = now - self.fetched_at > Duration::days(30);
        short || missing_today || stale
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diyanet::parse_page;

    fn fixture_days() -> Vec<DayTimes> {
        parse_page(
            &std::fs::read_to_string(format!(
                "{}/tests/fixtures/diyanet-9541-istanbul-2026-11-23.html",
                env!("CARGO_MANIFEST_DIR")
            ))
            .unwrap(),
        )
    }

    #[test]
    fn roundtrip_merge_refresh() {
        let dir = tempfile::tempdir().unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 11, 23).unwrap();
        let now = today.and_hms_opt(9, 0, 0).unwrap().and_utc();
        let mut c = TimesCache::new(9541);
        assert!(c.needs_refresh(today, now));
        c.merge(fixture_days(), now, today);
        // 40 günden eski günler atıldı
        assert_eq!(c.days.first().unwrap().date, today - Duration::days(40));
        assert_eq!(c.covered_until(), NaiveDate::from_ymd_opt(2026, 12, 31));
        assert!(!c.needs_refresh(today, now));
        // 20 Aralık: 31 Aralık'a 11 gün kaldı → yenile
        let dec20 = NaiveDate::from_ymd_opt(2026, 12, 20).unwrap();
        assert!(c.needs_refresh(dec20, now + Duration::days(27)));
        // 31 gün sonra bayat
        assert!(c.needs_refresh(today, now + Duration::days(31)));
        c.save(dir.path()).unwrap();
        let back = TimesCache::load(dir.path(), 9541).unwrap();
        assert_eq!(back, c);
        assert!(TimesCache::load(dir.path(), 1).is_none());
        assert_eq!(back.get(today).unwrap().hijri, "14 Cemaziyelahir 1448");
    }
}

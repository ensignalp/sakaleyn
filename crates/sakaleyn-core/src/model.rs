//! Temel tipler: vakitler ve bir günün vakit tablosu.

use chrono::{NaiveDate, NaiveTime};
use serde::{Deserialize, Serialize};

/// Diyanet tablosundaki altı vakit (sırası tablo sütunlarıyla aynı).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Vakit {
    Imsak,
    Gunes,
    Ogle,
    Ikindi,
    Aksam,
    Yatsi,
}

impl Vakit {
    pub const ALL: [Vakit; 6] = [
        Vakit::Imsak,
        Vakit::Gunes,
        Vakit::Ogle,
        Vakit::Ikindi,
        Vakit::Aksam,
        Vakit::Yatsi,
    ];
    /// Ezan okunan vakitler (güneş hariç).
    pub const ADHAN: [Vakit; 5] = [Vakit::Imsak, Vakit::Ogle, Vakit::Ikindi, Vakit::Aksam, Vakit::Yatsi];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn key(self) -> &'static str {
        match self {
            Vakit::Imsak => "imsak",
            Vakit::Gunes => "gunes",
            Vakit::Ogle => "ogle",
            Vakit::Ikindi => "ikindi",
            Vakit::Aksam => "aksam",
            Vakit::Yatsi => "yatsi",
        }
    }

    pub fn from_key(s: &str) -> Option<Vakit> {
        Vakit::ALL.into_iter().find(|v| v.key() == s)
    }

    pub fn name(self, lang: Lang) -> &'static str {
        match (lang, self) {
            (Lang::Tr, Vakit::Imsak) => "İmsak",
            (Lang::Tr, Vakit::Gunes) => "Güneş",
            (Lang::Tr, Vakit::Ogle) => "Öğle",
            (Lang::Tr, Vakit::Ikindi) => "İkindi",
            (Lang::Tr, Vakit::Aksam) => "Akşam",
            (Lang::Tr, Vakit::Yatsi) => "Yatsı",
            (Lang::En, Vakit::Imsak) => "Fajr",
            (Lang::En, Vakit::Gunes) => "Sunrise",
            (Lang::En, Vakit::Ogle) => "Dhuhr",
            (Lang::En, Vakit::Ikindi) => "Asr",
            (Lang::En, Vakit::Aksam) => "Maghrib",
            (Lang::En, Vakit::Yatsi) => "Isha",
        }
    }

    pub fn has_adhan(self) -> bool {
        self != Vakit::Gunes
    }

    pub fn next(self) -> Option<Vakit> {
        Vakit::ALL.get(self.index() + 1).copied()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    #[default]
    Tr,
    En,
}

impl Lang {
    /// `"auto"` → `$LANGUAGE`/`$LC_ALL`/`$LC_MESSAGES`/`$LANG` Türkçe değilse İngilizce.
    pub fn resolve(setting: &str) -> Lang {
        match setting {
            "tr" => Lang::Tr,
            "en" => Lang::En,
            _ => {
                let env = ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"]
                    .iter()
                    .filter_map(|k| std::env::var(k).ok())
                    .find(|v| !v.is_empty())
                    .unwrap_or_default();
                if env.is_empty() || env.starts_with("tr") {
                    Lang::Tr
                } else {
                    Lang::En
                }
            }
        }
    }
}

/// Bir günün vakitleri (ilçenin yerel saatiyle, Diyanet'in yazdığı gibi).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DayTimes {
    pub date: NaiveDate,
    /// Diyanet'in yazdığı hicri tarih, ör. `"14 Cemaziyelahir 1448"`.
    #[serde(default)]
    pub hijri: String,
    #[serde(with = "hhmm6")]
    pub times: [NaiveTime; 6],
}

impl DayTimes {
    pub fn time(&self, v: Vakit) -> NaiveTime {
        self.times[v.index()]
    }

    /// Hicri ay adı Ramazan mı?
    pub fn is_ramadan(&self) -> bool {
        crate::text::fold(&self.hijri).contains("ramazan")
    }
}

/// Vakitlerin kaynağı.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// Diyanet tablosu (önbellekten ya da yeni indirilmiş).
    Diyanet,
    /// Diyanet verisi yok: astronomik yedek hesap ("tahmini").
    Calculated,
}

/// `[NaiveTime; 6]` ↔ `["06:24", …]`
mod hhmm6 {
    use chrono::NaiveTime;
    use serde::de::Error;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(t: &[NaiveTime; 6], s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let mut seq = s.serialize_seq(Some(6))?;
        for x in t {
            seq.serialize_element(&x.format("%H:%M").to_string())?;
        }
        seq.end()
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[NaiveTime; 6], D::Error> {
        let v: Vec<String> = Vec::deserialize(d)?;
        if v.len() != 6 {
            return Err(D::Error::custom("expected 6 times"));
        }
        let mut out = [NaiveTime::MIN; 6];
        for (i, s) in v.iter().enumerate() {
            out[i] = NaiveTime::parse_from_str(s, "%H:%M").map_err(D::Error::custom)?;
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daytimes_roundtrip() {
        let d = DayTimes {
            date: NaiveDate::from_ymd_opt(2026, 3, 1).unwrap(),
            hijri: "12 Ramazan 1447".into(),
            times: [NaiveTime::from_hms_opt(5, 1, 0).unwrap(); 6],
        };
        let j = serde_json::to_string(&d).unwrap();
        assert!(j.contains("\"05:01\""));
        assert_eq!(serde_json::from_str::<DayTimes>(&j).unwrap(), d);
        assert!(d.is_ramadan());
    }

    #[test]
    fn vakit_keys() {
        for v in Vakit::ALL {
            assert_eq!(Vakit::from_key(v.key()), Some(v));
        }
        assert_eq!(serde_json::to_string(&Vakit::Ikindi).unwrap(), "\"ikindi\"");
    }
}

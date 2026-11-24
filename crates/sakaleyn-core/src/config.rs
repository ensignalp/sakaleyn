//! `~/.config/sakaleyn/config.toml`. Her alan isteğe bağlıdır; eksikler varsayılan.
//!
//! Vakit anahtarları: `imsak`, `gunes`, `ogle`, `ikindi`, `aksam`, `yatsi`.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::Vakit;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// `"tr"` (varsayılan), `"en"` ya da `"auto"` (sistem dili).
    pub language: String,
    /// Ana anahtar: kapalıyken hiçbir uyarı, ezan ya da bildirim gelmez (Hızlı Ayarlar kutucuğu).
    pub enabled: bool,
    pub location: LocationCfg,
    pub alerts: AlertsCfg,
    pub adhan: AdhanCfg,
    /// Vakit başına elle düzeltme (dakika, ±60).
    pub offsets: BTreeMap<String, i32>,
    pub friday: FridayCfg,
    pub ramadan: RamadanCfg,
    pub sunrise: SunriseCfg,
    pub kerahat: KerahatCfg,
    pub end_reminder: EndReminderCfg,
    pub religious_days: ReligiousCfg,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LocationMode {
    #[default]
    Auto,
    Manual,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LocationCfg {
    pub mode: LocationMode,
    /// Elle seçilen (ya da otomatik bulunan son) Diyanet ilçe kimliği.
    pub district_id: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AlertsCfg {
    pub enabled: bool,
    /// Vakitten kaç dakika önce uyarılsın (1–180; 0 kapalı).
    pub minutes: u32,
    /// Vakit başına farklı süre (`imsak = 60`).
    pub per_vakit: BTreeMap<String, u32>,
    /// Uyarı verilecek vakitler.
    pub vakits: Vec<Vakit>,
    pub sound: String,
    pub volume: f64,
    /// Rahatsız Etmeyin açıkken de ses çalsın mı (bildirim her zaman gelir).
    pub play_in_dnd: bool,
    pub snooze_minutes: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AdhanCfg {
    pub enabled: bool,
    /// Ezan okunacak vakitler (diğerlerinde yalnızca bildirim).
    pub vakits: Vec<Vakit>,
    /// Vakit başına ses kimliği.
    pub sounds: BTreeMap<String, String>,
    pub volume: f64,
    /// Ezan boyunca çalan medya oynatıcılarını (MPRIS) duraklat.
    pub pause_media: bool,
    pub play_in_dnd: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FridayCfg {
    pub enabled: bool,
    pub minutes: u32,
    /// Cuma öğle ezanı için ayrı ses (boşsa normal öğle sesi).
    pub sound: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RamadanCfg {
    pub enabled: bool,
    /// Sahur uyarısı: imsaktan kaç dakika önce (0 kapalı). Ramazan'da imsak ön uyarısının yerini alır.
    pub sahur_minutes: u32,
    pub sahur_sound: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SunriseCfg {
    /// Güneş doğmadan önce "sabah namazı vakti çıkıyor" uyarısı.
    pub alert: bool,
    pub minutes: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct KerahatCfg {
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EndReminderCfg {
    /// Vakit çıkmadan önce, "Kıldım" denmediyse hatırlat.
    pub enabled: bool,
    pub minutes: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReligiousCfg {
    pub enabled: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            language: "tr".into(),
            enabled: true,
            location: LocationCfg::default(),
            alerts: AlertsCfg::default(),
            adhan: AdhanCfg::default(),
            offsets: BTreeMap::new(),
            friday: FridayCfg::default(),
            ramadan: RamadanCfg::default(),
            sunrise: SunriseCfg::default(),
            kerahat: KerahatCfg::default(),
            end_reminder: EndReminderCfg::default(),
            religious_days: ReligiousCfg::default(),
        }
    }
}

impl Default for AlertsCfg {
    fn default() -> Self {
        AlertsCfg {
            enabled: true,
            minutes: 45,
            per_vakit: BTreeMap::new(),
            vakits: Vakit::ADHAN.to_vec(),
            sound: "nur".into(),
            volume: 0.6,
            play_in_dnd: false,
            snooze_minutes: 5,
        }
    }
}

pub const DEFAULT_ADHAN_SOUNDS: [(Vakit, &str); 5] = [
    (Vakit::Imsak, "alafasy-fajr"),
    (Vakit::Ogle, "cosar-ussak"),
    (Vakit::Ikindi, "cosar-rast"),
    (Vakit::Aksam, "kabe"),
    (Vakit::Yatsi, "kassas-medine"),
];

impl Default for AdhanCfg {
    fn default() -> Self {
        AdhanCfg {
            enabled: true,
            vakits: Vakit::ADHAN.to_vec(),
            sounds: DEFAULT_ADHAN_SOUNDS
                .iter()
                .map(|(v, s)| (v.key().to_string(), s.to_string()))
                .collect(),
            volume: 0.85,
            pause_media: true,
            play_in_dnd: true,
        }
    }
}

impl Default for FridayCfg {
    fn default() -> Self {
        FridayCfg {
            enabled: true,
            minutes: 60,
            sound: "senol-ogle".into(),
        }
    }
}

impl Default for RamadanCfg {
    fn default() -> Self {
        RamadanCfg {
            enabled: true,
            sahur_minutes: 45,
            sahur_sound: "uyandirma".into(),
        }
    }
}

impl Default for SunriseCfg {
    fn default() -> Self {
        SunriseCfg {
            alert: false,
            minutes: 30,
        }
    }
}

impl Default for EndReminderCfg {
    fn default() -> Self {
        EndReminderCfg {
            enabled: false,
            minutes: 20,
        }
    }
}

impl Default for ReligiousCfg {
    fn default() -> Self {
        ReligiousCfg { enabled: true }
    }
}

impl Config {
    /// Dosyayı okur. Yoksa varsayılan; bozuksa varsayılan + uyarı metni.
    pub fn load_from(path: &Path) -> (Config, Option<String>) {
        match std::fs::read_to_string(path) {
            Ok(s) => match toml::from_str::<Config>(&s) {
                Ok(c) => (c.sanitized(), None),
                Err(e) => (Config::default(), Some(format!("{}: {e}", path.display()))),
            },
            Err(_) => (Config::default(), None),
        }
    }

    pub fn load() -> (Config, Option<String>) {
        Self::load_from(&crate::paths::config_file())
    }

    /// Atomik yazma (geçici dosya + rename).
    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let body = toml::to_string_pretty(self).map_err(std::io::Error::other)?;
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, format!("# Sakaleyn ayarları (ayarlar penceresi de bu dosyayı yazar)\n\n{body}"))?;
        std::fs::rename(tmp, path)
    }

    pub fn save(&self) -> std::io::Result<()> {
        self.save_to(&crate::paths::config_file())
    }

    /// JSON yaması uygular (`{"alerts": {"minutes": 30}}`): iç içe nesneler birleşir,
    /// diğer değerler (listeler dahil) yerine geçer, `null` anahtarı varsayılana döndürür.
    pub fn patched(&self, patch: &Value) -> Result<Config, String> {
        let mut base = serde_json::to_value(self).map_err(|e| e.to_string())?;
        merge(&mut base, patch);
        let c: Config = serde_json::from_value(base).map_err(|e| e.to_string())?;
        Ok(c.sanitized())
    }

    /// Aralık dışı değerleri sınırlar.
    pub fn sanitized(mut self) -> Config {
        self.alerts.minutes = self.alerts.minutes.min(180);
        for m in self.alerts.per_vakit.values_mut() {
            *m = (*m).min(180);
        }
        self.alerts.volume = self.alerts.volume.clamp(0.0, 1.0);
        self.alerts.snooze_minutes = self.alerts.snooze_minutes.clamp(1, 60);
        self.adhan.volume = self.adhan.volume.clamp(0.0, 1.0);
        self.friday.minutes = self.friday.minutes.min(240);
        self.ramadan.sahur_minutes = self.ramadan.sahur_minutes.min(180);
        self.sunrise.minutes = self.sunrise.minutes.clamp(1, 120);
        self.end_reminder.minutes = self.end_reminder.minutes.clamp(1, 120);
        for o in self.offsets.values_mut() {
            *o = (*o).clamp(-60, 60);
        }
        self.offsets.retain(|k, v| Vakit::from_key(k).is_some() && *v != 0);
        self.alerts.vakits.sort();
        self.alerts.vakits.dedup();
        self.adhan.vakits.sort();
        self.adhan.vakits.dedup();
        self.adhan.vakits.retain(|v| v.has_adhan());
        self
    }

    pub fn offset(&self, v: Vakit) -> i32 {
        self.offsets.get(v.key()).copied().unwrap_or(0)
    }

    /// Vakit için ön uyarı süresi (dakika); kapalıysa `None`.
    pub fn alert_minutes(&self, v: Vakit) -> Option<u32> {
        if !self.alerts.enabled || !self.alerts.vakits.contains(&v) {
            return None;
        }
        let m = self.alerts.per_vakit.get(v.key()).copied().unwrap_or(self.alerts.minutes);
        (m > 0).then_some(m)
    }

    /// Vakitte çalınacak ezan sesi; ezan kapalıysa `None` (yalnızca bildirim).
    pub fn adhan_sound(&self, v: Vakit) -> Option<String> {
        if !self.adhan.enabled || !self.adhan.vakits.contains(&v) {
            return None;
        }
        self.adhan.sounds.get(v.key()).cloned().or_else(|| {
            DEFAULT_ADHAN_SOUNDS
                .iter()
                .find(|(x, _)| *x == v)
                .map(|(_, s)| s.to_string())
        })
    }
}

fn merge(base: &mut Value, patch: &Value) {
    match (base, patch) {
        (Value::Object(b), Value::Object(p)) => {
            for (k, v) in p {
                // null: anahtarı kaldır (varsayılan değer geri gelir)
                if v.is_null() {
                    b.remove(k);
                    continue;
                }
                match b.get_mut(k) {
                    Some(bv) if bv.is_object() && v.is_object() => merge(bv, v),
                    _ => {
                        b.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        (b, p) => *b = p.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn defaults() {
        let c = Config::default();
        assert_eq!(c.alert_minutes(Vakit::Ogle), Some(45));
        assert_eq!(c.alert_minutes(Vakit::Gunes), None);
        assert_eq!(c.adhan_sound(Vakit::Ikindi).as_deref(), Some("cosar-rast"));
        assert_eq!(c.adhan_sound(Vakit::Gunes), None);
    }

    #[test]
    fn toml_roundtrip_and_partial_file() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("config.toml");
        std::fs::write(&p, "[alerts]\nminutes = 30\n[alerts.per_vakit]\nimsak = 60\n[offsets]\nogle = 2\n").unwrap();
        let (c, w) = Config::load_from(&p);
        assert!(w.is_none());
        assert_eq!(c.alert_minutes(Vakit::Ogle), Some(30));
        assert_eq!(c.alert_minutes(Vakit::Imsak), Some(60));
        assert_eq!(c.offset(Vakit::Ogle), 2);
        assert_eq!(c.adhan.volume, 0.85);
        c.save_to(&p).unwrap();
        assert_eq!(Config::load_from(&p).0, c);
    }

    /// config.example.toml varsayılanlarla birebir aynı kalmalı.
    #[test]
    fn example_file_matches_defaults() {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config.example.toml");
        let (c, w) = Config::load_from(&p);
        assert!(w.is_none(), "{w:?}");
        assert_eq!(c, Config::default());
    }

    #[test]
    fn broken_file_falls_back() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("config.toml");
        std::fs::write(&p, "alerts = 3 [").unwrap();
        let (c, w) = Config::load_from(&p);
        assert!(w.is_some());
        assert_eq!(c, Config::default());
    }

    #[test]
    fn patch_merges_and_clamps() {
        let c = Config::default()
            .patched(&json!({"alerts": {"minutes": 500, "volume": 2.0}, "adhan": {"vakits": ["ogle", "gunes"]}}))
            .unwrap();
        assert_eq!(c.alerts.minutes, 180);
        assert_eq!(c.alerts.volume, 1.0);
        assert_eq!(c.alerts.sound, "nur");
        assert_eq!(c.adhan.vakits, vec![Vakit::Ogle]);
        assert!(Config::default().patched(&json!({"alerts": {"minutes": "x"}})).is_err());
        let c = c.patched(&json!({"alerts": {"per_vakit": {"ogle": 10, "aksam": 5}}})).unwrap();
        let c = c.patched(&json!({"alerts": {"per_vakit": null}})).unwrap();
        assert!(c.alerts.per_vakit.is_empty());
        assert_eq!(c.alerts.minutes, 180);
    }
}

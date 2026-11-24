//! Bir günün olayları: ön uyarı, ezan, sahur, Cuma, güneş, kerahat, vakit çıkışı,
//! dini gün. Saf fonksiyonlar; zamanlayıcı (`sakaleyn` servisi) bunları sırayla ateşler.

use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveTime, TimeZone, Utc, Weekday};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::model::{DayTimes, Vakit};
use crate::religious::ReligiousDay;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    /// Vakitten X dk önce.
    PreAlert,
    /// Vakit girdi (ses varsa ezan okunur, yoksa yalnızca bildirim).
    Adhan,
    /// Ramazan: imsaktan X dk önce sahur uyarısı.
    Sahur,
    /// Cuma: öğleden X dk önce.
    Friday,
    /// Güneş doğmadan X dk önce: sabah namazı vakti çıkıyor.
    Sunrise,
    /// Kerahat vakti başladı (güneş doğunca / akşamdan 45 dk önce).
    Kerahat,
    /// Vakit çıkmadan X dk önce ("Kıldım" denmediyse).
    EndReminder,
    /// Kandil, bayram vb.
    ReligiousDay,
}

impl EventKind {
    pub fn key(self) -> &'static str {
        match self {
            EventKind::PreAlert => "pre_alert",
            EventKind::Adhan => "adhan",
            EventKind::Sahur => "sahur",
            EventKind::Friday => "friday",
            EventKind::Sunrise => "sunrise",
            EventKind::Kerahat => "kerahat",
            EventKind::EndReminder => "end_reminder",
            EventKind::ReligiousDay => "religious_day",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Event {
    /// Tekrar çalmayı önleyen kalıcı kimlik: `"2026-11-23:pre_alert:ogle"`.
    pub id: String,
    pub at: DateTime<Utc>,
    pub kind: EventKind,
    pub vakit: Option<Vakit>,
    /// Olayın ilgili olduğu vakit anı (ör. ön uyarıda vaktin kendisi).
    pub target: DateTime<Utc>,
    /// Çalınacak ses kimliği (yoksa yalnızca bildirim).
    pub sound: Option<String>,
    /// Dini gün adı vb.
    pub label: Option<String>,
    pub ramadan: bool,
    pub friday: bool,
}

/// Yerel tarih + saat → UTC. Yaz saati boşluğunda bir sonraki geçerli an.
pub fn to_utc(tz: Tz, date: NaiveDate, t: NaiveTime) -> DateTime<Utc> {
    let naive = date.and_time(t);
    match tz.from_local_datetime(&naive) {
        chrono::LocalResult::Single(x) => x.with_timezone(&Utc),
        chrono::LocalResult::Ambiguous(a, _) => a.with_timezone(&Utc),
        chrono::LocalResult::None => tz
            .from_local_datetime(&(naive + Duration::hours(1)))
            .earliest()
            .map(|x| x.with_timezone(&Utc))
            .unwrap_or_else(|| naive.and_utc()),
    }
}

/// Düzeltmeler uygulanmış altı vakit anı (UTC).
pub fn instants(day: &DayTimes, tz: Tz, cfg: &Config) -> [DateTime<Utc>; 6] {
    Vakit::ALL.map(|v| to_utc(tz, day.date, day.time(v)) + Duration::minutes(cfg.offset(v) as i64))
}

/// Günün bütün olayları, zamana göre sıralı. `next_day`: yatsıdan sonraki imsak için
/// (vakit çıkış hatırlatması); `religious`: o yılın dini günleri.
pub fn build_day(
    day: &DayTimes,
    tz: Tz,
    cfg: &Config,
    religious: &[ReligiousDay],
) -> Vec<Event> {
    let at = instants(day, tz, cfg);
    let ramadan = cfg.ramadan.enabled && day.is_ramadan();
    let friday = day.date.weekday() == Weekday::Fri;
    let mut out = Vec::new();
    let mut push = |kind: EventKind, vakit: Option<Vakit>, when: DateTime<Utc>, target: DateTime<Utc>, sound: Option<String>, label: Option<String>| {
        let id = match (&vakit, &label) {
            (Some(v), _) => format!("{}:{}:{}", day.date, kind.key(), v.key()),
            (None, Some(l)) => format!("{}:{}:{}", day.date, kind.key(), crate::text::fold(l)),
            _ => format!("{}:{}", day.date, kind.key()),
        };
        out.push(Event {
            id,
            at: when,
            kind,
            vakit,
            target,
            sound,
            label,
            ramadan,
            friday,
        });
    };

    for v in Vakit::ADHAN {
        let t = at[v.index()];
        // Vakit girişi: ezan (ses kapalıysa yalnızca bildirim).
        let mut sound = cfg.adhan_sound(v);
        if friday && v == Vakit::Ogle && cfg.friday.enabled && sound.is_some() && !cfg.friday.sound.is_empty() {
            sound = Some(cfg.friday.sound.clone());
        }
        push(EventKind::Adhan, Some(v), t, t, sound, None);

        // Ramazan'da imsak ön uyarısının yerine sahur uyarısı.
        if ramadan && v == Vakit::Imsak {
            if cfg.ramadan.sahur_minutes > 0 {
                let s = (!cfg.ramadan.sahur_sound.is_empty()).then(|| cfg.ramadan.sahur_sound.clone());
                push(EventKind::Sahur, Some(v), t - Duration::minutes(cfg.ramadan.sahur_minutes as i64), t, s, None);
            }
            continue;
        }
        let friday_alert = friday && v == Vakit::Ogle && cfg.friday.enabled && cfg.friday.minutes > 0;
        if friday_alert {
            push(EventKind::Friday, Some(v), t - Duration::minutes(cfg.friday.minutes as i64), t, Some(cfg.alerts.sound.clone()), None);
        }
        if let Some(m) = cfg.alert_minutes(v) {
            // Cuma uyarısıyla aynı ana denk gelen ön uyarı tekrar edilmez.
            if !(friday_alert && m == cfg.friday.minutes) {
                push(EventKind::PreAlert, Some(v), t - Duration::minutes(m as i64), t, Some(cfg.alerts.sound.clone()), None);
            }
        }
    }

    let gunes = at[Vakit::Gunes.index()];
    if cfg.sunrise.alert {
        push(EventKind::Sunrise, Some(Vakit::Gunes), gunes - Duration::minutes(cfg.sunrise.minutes as i64), gunes, Some(cfg.alerts.sound.clone()), None);
    }
    if cfg.kerahat.enabled {
        let aksam = at[Vakit::Aksam.index()];
        push(EventKind::Kerahat, Some(Vakit::Gunes), gunes, gunes + Duration::minutes(45), None, None);
        push(EventKind::Kerahat, Some(Vakit::Ikindi), aksam - Duration::minutes(45), aksam, None, None);
    }
    if cfg.end_reminder.enabled {
        // imsak→güneş, öğle→ikindi, ikindi→akşam, akşam→yatsı (yatsı sabaha kadar sürer).
        for v in [Vakit::Imsak, Vakit::Ogle, Vakit::Ikindi, Vakit::Aksam] {
            let end = at[v.next().unwrap().index()];
            let when = end - Duration::minutes(cfg.end_reminder.minutes as i64);
            if when > at[v.index()] {
                push(EventKind::EndReminder, Some(v), when, end, Some(cfg.alerts.sound.clone()), None);
            }
        }
    }
    if cfg.religious_days.enabled {
        for r in religious.iter().filter(|r| r.date == day.date) {
            let nine = to_utc(tz, day.date, NaiveTime::from_hms_opt(9, 0, 0).unwrap());
            push(EventKind::ReligiousDay, None, nine, nine, None, Some(r.name.clone()));
        }
    }
    out.sort_by(|a, b| a.at.cmp(&b.at).then_with(|| a.id.cmp(&b.id)));
    out
}

/// Şu anki ve sıradaki vakit. `yesterday`/`tomorrow`: gece yarısı geçişleri için.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Position {
    /// Şu anki vakit ve başlangıcı (yatsıdan sonra gece yarısını geçince dünkü yatsı).
    pub current: Option<(Vakit, DateTime<Utc>)>,
    pub next: (Vakit, DateTime<Utc>),
}

pub fn position(
    now: DateTime<Utc>,
    yesterday: Option<&[DateTime<Utc>; 6]>,
    today: &[DateTime<Utc>; 6],
    tomorrow: Option<&[DateTime<Utc>; 6]>,
) -> Position {
    let mut current = yesterday.map(|y| (Vakit::Yatsi, y[Vakit::Yatsi.index()]));
    for v in Vakit::ALL {
        let t = today[v.index()];
        if t <= now {
            current = Some((v, t));
        } else {
            return Position { current, next: (v, t) };
        }
    }
    let next = match tomorrow {
        Some(t) => (Vakit::Imsak, t[0]),
        // yarının tablosu yoksa: bugünkü imsak + 1 gün (dakikalar içinde doğru)
        None => (Vakit::Imsak, today[0] + Duration::days(1)),
    };
    Position { current, next }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn t(s: &str) -> NaiveTime {
        NaiveTime::parse_from_str(s, "%H:%M").unwrap()
    }

    fn day(date: (i32, u32, u32), hijri: &str) -> DayTimes {
        DayTimes {
            date: NaiveDate::from_ymd_opt(date.0, date.1, date.2).unwrap(),
            hijri: hijri.into(),
            times: [t("06:24"), t("07:53"), t("12:55"), t("15:25"), t("17:47"), t("19:11")],
        }
    }

    const IST: Tz = chrono_tz::Europe::Istanbul;

    fn local(e: &Event) -> String {
        e.at.with_timezone(&IST).format("%H:%M").to_string()
    }

    #[test]
    fn default_day() {
        // 23 Kasım 2026 Pazartesi
        let ev = build_day(&day((2026, 11, 23), "14 Cemaziyelahir 1448"), IST, &Config::default(), &[]);
        let list: Vec<_> = ev.iter().map(|e| (local(e), e.kind, e.vakit)).collect();
        assert_eq!(ev.len(), 10, "{list:?}");
        assert_eq!(list[0], ("05:39".into(), EventKind::PreAlert, Some(Vakit::Imsak)));
        assert_eq!(list[1], ("06:24".into(), EventKind::Adhan, Some(Vakit::Imsak)));
        assert_eq!(list[2], ("12:10".into(), EventKind::PreAlert, Some(Vakit::Ogle)));
        assert_eq!(ev[1].sound.as_deref(), Some("alafasy-fajr"));
        assert_eq!(ev[0].id, "2026-11-23:pre_alert:imsak");
        // 06:24 İstanbul = 03:24 UTC
        assert_eq!(ev[1].at.format("%H:%M").to_string(), "03:24");
    }

    #[test]
    fn friday_and_ramadan() {
        let cfg = Config::default();
        // 27 Kasım 2026 Cuma
        let ev = build_day(&day((2026, 11, 27), ""), IST, &cfg, &[]);
        let fri: Vec<_> = ev.iter().filter(|e| e.kind == EventKind::Friday).collect();
        assert_eq!(fri.len(), 1);
        assert_eq!(local(fri[0]), "11:55");
        let ogle = ev.iter().find(|e| e.kind == EventKind::Adhan && e.vakit == Some(Vakit::Ogle)).unwrap();
        assert_eq!(ogle.sound.as_deref(), Some("senol-ogle"));
        assert!(ogle.friday);

        let ev = build_day(&day((2026, 3, 1), "12 Ramazan 1447"), IST, &cfg, &[]);
        assert!(ev.iter().all(|e| e.ramadan));
        assert!(!ev.iter().any(|e| e.kind == EventKind::PreAlert && e.vakit == Some(Vakit::Imsak)));
        let sahur = ev.iter().find(|e| e.kind == EventKind::Sahur).unwrap();
        assert_eq!(local(sahur), "05:39");
        assert_eq!(sahur.sound.as_deref(), Some("uyandirma"));
    }

    #[test]
    fn options_and_offsets() {
        let cfg = Config::default()
            .patched(&json!({
                "alerts": {"minutes": 0, "per_vakit": {"aksam": 10}},
                "adhan": {"vakits": ["aksam"]},
                "offsets": {"aksam": 2},
                "sunrise": {"alert": true, "minutes": 30},
                "kerahat": {"enabled": true},
                "end_reminder": {"enabled": true, "minutes": 20},
            }))
            .unwrap();
        let rel = [ReligiousDay {
            date: NaiveDate::from_ymd_opt(2026, 11, 23).unwrap(),
            name: "Test Kandili".into(),
            hijri: String::new(),
        }];
        let ev = build_day(&day((2026, 11, 23), ""), IST, &cfg, &rel);
        let pre: Vec<_> = ev.iter().filter(|e| e.kind == EventKind::PreAlert).collect();
        assert_eq!(pre.len(), 1);
        assert_eq!(local(pre[0]), "17:39"); // 17:47 + 2 − 10
        let adhans: Vec<_> = ev.iter().filter(|e| e.kind == EventKind::Adhan).collect();
        assert_eq!(adhans.len(), 5);
        assert_eq!(adhans.iter().filter(|e| e.sound.is_some()).count(), 1);
        assert!(ev.iter().any(|e| e.kind == EventKind::Sunrise && local(e) == "07:23"));
        assert_eq!(ev.iter().filter(|e| e.kind == EventKind::Kerahat).count(), 2);
        assert_eq!(ev.iter().filter(|e| e.kind == EventKind::EndReminder).count(), 4);
        let r = ev.iter().find(|e| e.kind == EventKind::ReligiousDay).unwrap();
        assert_eq!(local(r), "09:00");
        assert_eq!(r.id, "2026-11-23:religious_day:testkandili");
        assert!(ev.windows(2).all(|w| w[0].at <= w[1].at));
    }

    #[test]
    fn berlin_dst_and_position() {
        let tz = chrono_tz::Europe::Berlin;
        let d = day((2026, 7, 1), "");
        let at = instants(&d, tz, &Config::default());
        // yaz saati: UTC+2
        assert_eq!(at[0].format("%H:%M").to_string(), "04:24");

        let today = instants(&day((2026, 11, 23), ""), IST, &Config::default());
        let utc = |h, m| to_utc(IST, NaiveDate::from_ymd_opt(2026, 11, 23).unwrap(), NaiveTime::from_hms_opt(h, m, 0).unwrap());
        let p = position(utc(13, 0), None, &today, None);
        assert_eq!(p.current.unwrap().0, Vakit::Ogle);
        assert_eq!(p.next.0, Vakit::Ikindi);
        let p = position(utc(2, 0), Some(&today), &today, None);
        assert_eq!(p.current.unwrap().0, Vakit::Yatsi);
        assert_eq!(p.next.0, Vakit::Imsak);
        let p = position(utc(23, 0), None, &today, None);
        assert_eq!(p.next.0, Vakit::Imsak);
        assert_eq!(p.next.1 - today[0], Duration::days(1));
    }
}

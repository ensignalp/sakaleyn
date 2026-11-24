//! Bildirim ve CLI metinleri (Türkçe / İngilizce).

use sakaleyn_core::model::{Lang, Vakit};
use sakaleyn_core::schedule::{Event, EventKind};

pub fn t(lang: Lang, tr: &str, en: &str) -> String {
    match lang {
        Lang::Tr => tr.to_string(),
        Lang::En => en.to_string(),
    }
}

/// "45 dakika", "1 saat 5 dakika" / "45 minutes", "1 h 5 min".
pub fn duration(lang: Lang, minutes: i64) -> String {
    let m = minutes.max(0);
    let (h, r) = (m / 60, m % 60);
    match (lang, h, r) {
        (Lang::Tr, 0, r) => format!("{r} dakika"),
        (Lang::Tr, h, 0) => format!("{h} saat"),
        (Lang::Tr, h, r) => format!("{h} saat {r} dakika"),
        (Lang::En, 0, 1) => "1 minute".into(),
        (Lang::En, 0, r) => format!("{r} minutes"),
        (Lang::En, h, 0) => format!("{h} h"),
        (Lang::En, h, r) => format!("{h} h {r} min"),
    }
}

/// Türkçe "-e/-a" hal eki (vakit adlarına): "Öğle vaktine", "İkindi vaktine".
fn vakit_dative(lang: Lang, v: Vakit) -> String {
    match lang {
        Lang::Tr => format!("{} vaktine", v.name(lang)),
        Lang::En => v.name(lang).to_string(),
    }
}

/// Olay bildirimi: (başlık, gövde). `remaining`: hedef vakte kalan dakika,
/// `clock`: hedef vaktin yerel saati ("12:55"), `with_sound`: ezan okunuyor mu.
pub fn event_text(lang: Lang, e: &Event, remaining: i64, clock: &str, with_sound: bool) -> (String, String) {
    let v = e.vakit.unwrap_or(Vakit::Ogle);
    let d = duration(lang, remaining);
    match (e.kind, lang) {
        (EventKind::PreAlert, Lang::Tr) => (
            format!("{} {d} kaldı", vakit_dative(lang, v)),
            format!("{} ezanı: {clock}", v.name(lang)),
        ),
        (EventKind::PreAlert, Lang::En) => (format!("{} in {d}", v.name(lang)), format!("Adhan at {clock}.")),
        (EventKind::Adhan, _) => {
            let iftar = e.ramadan && v == Vakit::Aksam;
            let imsak_ramadan = e.ramadan && v == Vakit::Imsak;
            let title = match (lang, iftar, imsak_ramadan) {
                (Lang::Tr, true, _) => "İftar vakti".to_string(),
                (Lang::En, true, _) => "Iftar time".to_string(),
                (Lang::Tr, _, true) => "İmsak vakti: oruç başladı".to_string(),
                (Lang::En, _, true) => "Fajr: the fast has begun".to_string(),
                (Lang::Tr, _, _) if e.friday && v == Vakit::Ogle => "Cuma vakti".to_string(),
                (Lang::En, _, _) if e.friday && v == Vakit::Ogle => "Jumu'ah".to_string(),
                (Lang::Tr, _, _) => format!("{} vakti", v.name(lang)),
                (Lang::En, _, _) => format!("{} time", v.name(lang)),
            };
            let body = match (lang, with_sound, iftar) {
                (Lang::Tr, _, true) => format!("Hayırlı iftarlar · {clock}"),
                (Lang::En, _, true) => format!("Have a blessed iftar · {clock}"),
                (Lang::Tr, true, _) => format!("Ezan okunuyor · {clock}"),
                (Lang::En, true, _) => format!("Adhan playing · {clock}"),
                (Lang::Tr, false, _) => format!("Vakit girdi · {clock}"),
                (Lang::En, false, _) => format!("Prayer time has begun · {clock}"),
            };
            (title, body)
        }
        (EventKind::Sahur, Lang::Tr) => (format!("Sahur: imsaka {d}"), format!("İmsak: {clock}")),
        (EventKind::Sahur, Lang::En) => (format!("Suhoor: Fajr in {d}"), format!("Fajr at {clock}.")),
        (EventKind::Friday, Lang::Tr) => (format!("Cuma namazına {d}"), format!("Öğle ezanı: {clock}")),
        (EventKind::Friday, Lang::En) => (format!("Jumu'ah in {d}"), format!("Dhuhr adhan at {clock}.")),
        (EventKind::Sunrise, Lang::Tr) => (
            format!("Güneşin doğmasına {d}"),
            format!("Sabah namazı vaktinin sonu: {clock}"),
        ),
        (EventKind::Sunrise, Lang::En) => (format!("Sunrise in {d}"), format!("Fajr time ends at {clock}.")),
        (EventKind::Kerahat, Lang::Tr) if v == Vakit::Gunes => (
            "Kerahat vakti".into(),
            "Güneş doğdu; yaklaşık 45 dakika namaz kılınmaz.".into(),
        ),
        (EventKind::Kerahat, Lang::Tr) => (
            "Kerahat vakti".into(),
            format!("Akşama {d}; güneş batana kadar kerahat vakti."),
        ),
        (EventKind::Kerahat, Lang::En) if v == Vakit::Gunes => (
            "Makruh time".into(),
            "The sun has risen; no prayer for about 45 minutes.".into(),
        ),
        (EventKind::Kerahat, Lang::En) => ("Makruh time".into(), format!("Maghrib in {d}; makruh until sunset.")),
        (EventKind::EndReminder, Lang::Tr) => (
            format!("{} vakti çıkıyor", v.name(lang)),
            format!("Vaktin çıkmasına {d} ({clock})."),
        ),
        (EventKind::EndReminder, Lang::En) => (
            format!("{} time is ending", v.name(lang)),
            format!("Ends in {d} ({clock})."),
        ),
        (EventKind::ReligiousDay, _) => {
            let name = e.label.clone().unwrap_or_default();
            let night = sakaleyn_core::text::fold(&name).contains("kandil")
                || sakaleyn_core::text::fold(&name).contains("kadir");
            match (lang, night) {
                (Lang::Tr, true) => (format!("Bu gece {name}"), "Hayırlı kandiller.".into()),
                (Lang::Tr, false) => (format!("Bugün {name}"), String::new()),
                (Lang::En, true) => (format!("Tonight: {name}"), String::new()),
                (Lang::En, false) => (format!("Today: {name}"), String::new()),
            }
        }
    }
}

/// Uykudayken kaçırılan vakit.
pub fn missed_text(lang: Lang, v: Vakit, clock: &str) -> (String, String) {
    match lang {
        Lang::Tr => (
            format!("{} vakti girdi ({clock})", v.name(lang)),
            "Bilgisayar o sırada uykudaydı.".into(),
        ),
        Lang::En => (format!("{} began at {clock}", v.name(lang)), "The computer was asleep.".into()),
    }
}

pub fn action_label(lang: Lang, key: &str, snooze: u32) -> String {
    match (key, lang) {
        ("stop", Lang::Tr) => "Sustur".into(),
        ("stop", Lang::En) => "Stop".into(),
        ("prayed", Lang::Tr) => "Kıldım".into(),
        ("prayed", Lang::En) => "Prayed".into(),
        ("snooze", Lang::Tr) => format!("{snooze} dk ertele"),
        ("snooze", Lang::En) => format!("Snooze {snooze} min"),
        (_, Lang::Tr) => "Kapat".into(),
        (_, Lang::En) => "Dismiss".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn ev(kind: EventKind, v: Vakit) -> Event {
        Event {
            id: String::new(),
            at: Utc::now(),
            kind,
            vakit: Some(v),
            target: Utc::now(),
            sound: None,
            label: None,
            ramadan: false,
            friday: false,
        }
    }

    #[test]
    fn texts() {
        assert_eq!(duration(Lang::Tr, 45), "45 dakika");
        assert_eq!(duration(Lang::Tr, 65), "1 saat 5 dakika");
        assert_eq!(duration(Lang::En, 1), "1 minute");
        let (t, b) = event_text(Lang::Tr, &ev(EventKind::PreAlert, Vakit::Ogle), 45, "12:55", true);
        assert_eq!(t, "Öğle vaktine 45 dakika kaldı");
        assert_eq!(b, "Öğle ezanı: 12:55");
        let mut e = ev(EventKind::Adhan, Vakit::Aksam);
        e.ramadan = true;
        assert_eq!(event_text(Lang::Tr, &e, 0, "17:47", true).0, "İftar vakti");
        assert_eq!(event_text(Lang::En, &ev(EventKind::Adhan, Vakit::Ikindi), 0, "15:25", false).1, "Prayer time has begun · 15:25");
    }
}

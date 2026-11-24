//! Diyanet'e ulaşılamadığında yedek: astronomik vakit hesabı.
//!
//! Diyanet yöntemi: imsak güneş ufkun 18° altındayken, yatsı 17°, ikindi
//! asr-ı evvel (gölge = boy + öğle gölgesi), güneş doğuş/batışta 0,833° (kırılma +
//! yarıçap). Üzerine Diyanet'in temkin farkları eklenir. Temkinler kayıtlı Diyanet
//! tablolarına (İstanbul, Ankara, Van, Edirne, Berlin) karşı ölçülmüştür; testler
//! sapmanın ±2 dk içinde kaldığını doğrular. Sonuç her zaman "tahmini" diye gösterilir.

use chrono::{Datelike, NaiveDate, NaiveTime, Offset, TimeZone, Timelike};
use chrono_tz::Tz;

use crate::model::DayTimes;

/// Diyanet temkinleri (dakika): imsak, güneş, öğle, ikindi, akşam, yatsı.
const TEMKIN: [f64; 6] = [TEMKIN_IMSAK, TEMKIN_GUNES, TEMKIN_OGLE, TEMKIN_IKINDI, TEMKIN_AKSAM, TEMKIN_YATSI];
const TEMKIN_IMSAK: f64 = 0.0;
const TEMKIN_GUNES: f64 = -7.0;
const TEMKIN_OGLE: f64 = 5.0;
const TEMKIN_IKINDI: f64 = 4.0;
const TEMKIN_AKSAM: f64 = 7.0;
const TEMKIN_YATSI: f64 = 0.0;

const FAJR_ANGLE: f64 = 18.0;
const ISHA_ANGLE: f64 = 17.0;
const HORIZON: f64 = 0.833;

fn rad(d: f64) -> f64 {
    d.to_radians()
}
fn deg(r: f64) -> f64 {
    r.to_degrees()
}

/// Güneşin sapma açısı (derece) ve zaman denklemi (saat); `jd`: Jülyen günü.
fn sun_position(jd: f64) -> (f64, f64) {
    let d = jd - 2451545.0;
    let g = (357.529 + 0.98560028 * d).rem_euclid(360.0);
    let q = (280.459 + 0.98564736 * d).rem_euclid(360.0);
    let l = (q + 1.915 * rad(g).sin() + 0.020 * rad(2.0 * g).sin()).rem_euclid(360.0);
    let e = 23.439 - 0.00000036 * d;
    let ra = deg((rad(e).cos() * rad(l).sin()).atan2(rad(l).cos())) / 15.0;
    let eqt = q / 15.0 - ra.rem_euclid(24.0);
    let eqt = (eqt + 12.0).rem_euclid(24.0) - 12.0;
    let decl = deg((rad(e).sin() * rad(l).sin()).asin());
    (decl, eqt)
}

fn julian(date: NaiveDate) -> f64 {
    let (mut y, mut m) = (date.year() as f64, date.month() as f64);
    let d = date.day() as f64;
    if m <= 2.0 {
        y -= 1.0;
        m += 12.0;
    }
    let a = (y / 100.0).floor();
    let b = 2.0 - a + (a / 4.0).floor();
    (365.25 * (y + 4716.0)).floor() + (30.6001 * (m + 1.0)).floor() + d + b - 1524.5
}

/// Güneşin ufkun `angle` derece altına indiği an ile öğle arasındaki süre (saat).
fn hour_angle(angle: f64, lat: f64, decl: f64) -> Option<f64> {
    let x = (-rad(angle).sin() - rad(decl).sin() * rad(lat).sin()) / (rad(decl).cos() * rad(lat).cos());
    (-1.0..=1.0).contains(&x).then(|| deg(x.acos()) / 15.0)
}

/// Asr-ı evvel: gölge boyu = cisim boyu + öğle gölgesi.
fn asr_angle(lat: f64, decl: f64) -> f64 {
    -deg((1.0 / (1.0 + rad((lat - decl).abs()).tan())).atan())
}

/// Yüksek enlem kuralı: imsak/yatsı alacakaranlığı gecenin `açı / BÖLEN` kadarını
/// geçemez (yazın güneş 18° altına hiç inmeyen yerler). Diyanet'in kesin kuralı
/// bilinmiyor; bölen Berlin tablosuna göre ayarlı ve orada imsak/yatsı yazın
/// ±10 dk sapabilir. 48° altında (Türkiye dahil) bu kural devreye girmez.
const HIGH_LAT_DIVISOR: f64 = 82.0;

/// Bir günün vakitleri (UTC saati, ondalık).
fn utc_hours(date: NaiveDate, lat: f64, lon: f64) -> [f64; 6] {
    utc_hours_with(date, lat, lon, HIGH_LAT_DIVISOR)
}

fn utc_hours_with(date: NaiveDate, lat: f64, lon: f64, divisor: f64) -> [f64; 6] {
    let jd = julian(date) + 0.5 - lon / 360.0; // yerel öğle civarı
    let (decl, eqt) = sun_position(jd);
    let noon = 12.0 - lon / 15.0 - eqt;
    let rise = hour_angle(HORIZON, lat, decl).unwrap_or(if decl * lat > 0.0 { 11.99 } else { 0.01 });
    let night = 24.0 - 2.0 * rise;
    let twilight = |angle: f64| {
        let max = night * angle / divisor;
        match hour_angle(angle, lat, decl) {
            Some(h) if lat.abs() <= 48.0 || h - rise <= max => h,
            _ => rise + max,
        }
    };
    let fajr = twilight(FAJR_ANGLE);
    let isha = twilight(ISHA_ANGLE);
    let asr = hour_angle(asr_angle(lat, decl), lat, decl).unwrap_or(rise * 0.6);
    [noon - fajr, noon - rise, noon, noon + asr, noon + rise, noon + isha]
}

/// Ham hesap + temkin, ilçenin yerel saatine çevrilmiş ve dakikaya yuvarlanmış.
pub fn day(date: NaiveDate, lat: f64, lon: f64, tz: Tz) -> DayTimes {
    let noon_utc = date.and_hms_opt(12, 0, 0).unwrap().and_utc();
    let offset_h = tz.offset_from_utc_datetime(&noon_utc.naive_utc()).fix().local_minus_utc() as f64 / 3600.0;
    let hours = utc_hours(date, lat, lon);
    to_local(date, hours, offset_h)
}

fn to_local(date: NaiveDate, hours: [f64; 6], offset_h: f64) -> DayTimes {
    let mut times = [NaiveTime::MIN; 6];
    for i in 0..6 {
        let local = hours[i] + offset_h + TEMKIN[i] / 60.0;
        let mins = (local * 60.0).round().rem_euclid(24.0 * 60.0) as u32;
        times[i] = NaiveTime::from_hms_opt(mins / 60, mins % 60, 0).unwrap();
    }
    // Sıra bozulursa (aşırı enlem) monoton olacak şekilde düzelt.
    for i in 1..6 {
        if times[i] <= times[i - 1] {
            let m = times[i - 1].num_seconds_from_midnight() / 60 + 1;
            times[i] = NaiveTime::from_hms_opt((m / 60) % 24, m % 60, 0).unwrap();
        }
    }
    DayTimes {
        date,
        hijri: String::new(),
        times,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::needless_range_loop)]
    #[ignore = "temkin/bölen ayarı için: cargo test fit_divisor -- --ignored --nocapture"]
    fn fit_divisor() {
        let d = by_id(11002).unwrap();
        let (lat, lon) = d.coords().unwrap();
        let days = parse_page(&fixture("diyanet-11002-berlin-2026-11-23.html"));
        for k in [60.0, 70.0, 80.0, 85.0, 88.0, 90.0, 92.0, 95.0, 100.0] {
            let mut worst = [0i32; 6];
            for want in &days {
                let noon_utc = want.date.and_hms_opt(12, 0, 0).unwrap().and_utc();
                let off = d.tz().offset_from_utc_datetime(&noon_utc.naive_utc()).fix().local_minus_utc() as f64 / 3600.0;
                let got = to_local(want.date, utc_hours_with(want.date, lat, lon, k), off);
                for i in 0..6 {
                    let diff = mins(got.times[i]) - mins(want.times[i]);
                    if diff.abs() > worst[i].abs() {
                        worst[i] = diff;
                    }
                }
            }
            eprintln!("k={k}: worst {worst:?}");
        }
    }
    use crate::diyanet::parse_page;
    use crate::districts::by_id;

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    fn mins(t: NaiveTime) -> i32 {
        (t.num_seconds_from_midnight() / 60) as i32
    }

    /// Yedek hesap bütün yıl boyunca Diyanet tablosundan en fazla ±2 dk sapar
    /// (48° üstünde imsak/yatsı için ±10 dk; bkz. HIGH_LAT_DIVISOR).
    #[test]
    fn matches_diyanet_within_two_minutes() {
        for (file, id) in [
            ("diyanet-9541-istanbul-2026-11-23.html", 9541),
            ("diyanet-9206-ankara-2026-11-23.html", 9206),
            ("diyanet-9930-van-2026-11-23.html", 9930),
            ("diyanet-9419-edirne-2026-11-23.html", 9419),
            ("diyanet-11002-berlin-2026-11-23.html", 11002),
        ] {
            let d = by_id(id).unwrap();
            let (lat, lon) = d.coords().unwrap();
            let mut worst = [0i32; 6];
            let mut sum = [0i64; 6];
            let days = parse_page(&fixture(file));
            for want in &days {
                let got = day(want.date, lat, lon, d.tz());
                for i in 0..6 {
                    let diff = mins(got.times[i]) - mins(want.times[i]);
                    sum[i] += diff as i64;
                    if diff.abs() > worst[i].abs() {
                        worst[i] = diff;
                    }
                }
            }
            let mean: Vec<f64> = sum.iter().map(|s| *s as f64 / days.len() as f64).collect();
            eprintln!("{file}: mean {mean:.2?} worst {worst:?}");
            for i in 0..6 {
                let tol = if lat > 48.0 && (i == 0 || i == 5) { 10 } else { 2 };
                assert!(worst[i].abs() <= tol, "{file}: vakit {i} worst diff {} min (mean {:.2})", worst[i], mean[i]);
            }
        }
    }
}

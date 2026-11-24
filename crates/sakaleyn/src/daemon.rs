//! `sakaleyn daemon`: vakitleri izleyen, ön uyarı ve ezanı çalan, durumu
//! D-Bus'ta yayınlayan servis.
//!
//! Zamanlayıcı kuralları:
//! - Uyku `tokio` saatini durdurduğu için en fazla 30 sn uyunur ve her uyanışta
//!   duvar saatine bakılır; logind "uyandım" deyince hemen bakılır.
//! - Vakti 2 dakikadan fazla geçmiş olay sesli çalınmaz. Ezan 60 dk içinde
//!   kaçırıldıysa sessiz "vakit girdi" bildirimi gelir, daha eskiyse hiçbir şey.
//! - Çalınan olaylar `~/.local/state/sakaleyn/state.json`'a yazılır; servis
//!   yeniden başlasa da aynı olay ikinci kez çalmaz.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::Duration as StdDuration;

use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};
use chrono_tz::Tz;
use sakaleyn_core::cache::TimesCache;
use sakaleyn_core::config::{Config, LocationMode};
use sakaleyn_core::districts::{self, District};
use sakaleyn_core::model::{DayTimes, Lang, Source, Vakit};
use sakaleyn_core::religious::{self, ReligiousDay};
use sakaleyn_core::schedule::{self, Event, EventKind};
use sakaleyn_core::{calc, diyanet};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::{Mutex, Notify, RwLock};
use zbus::object_server::SignalEmitter;

use crate::dbus::{BUS_NAME, OBJECT_PATH};
use crate::i18n;
use crate::notify::{self, Note};
use crate::sounds::{self, Player, Playing};
use crate::{geoclue, mpris, system};

/// Bu kadar gecikmiş olay hâlâ sesli çalınır.
const GRACE: i64 = 120;
/// Kaçırılan ezan için sessiz bildirim penceresi.
const MISSED_WINDOW: i64 = 60 * 60;
/// Zamanlayıcının en uzun uykusu.
const MAX_SLEEP: i64 = 30;

#[derive(Default, Serialize, Deserialize)]
struct Persist {
    /// Çalınmış olaylar (kimlik → an); 3 günden eskiler atılır.
    #[serde(default)]
    fired: BTreeMap<String, DateTime<Utc>>,
    /// Bu tarihte (ilçenin yerel tarihi) yalnızca bildirim.
    #[serde(default)]
    muted: Option<NaiveDate>,
    /// "Kıldım": `"2026-11-23:ogle"`.
    #[serde(default)]
    prayed: BTreeSet<String>,
    /// Ertelenmiş uyarılar.
    #[serde(default)]
    snoozed: Vec<Event>,
    /// "Konum bulunamadı" bildirimi gösterildi mi.
    #[serde(default)]
    location_notice: bool,
}

#[derive(Clone, Debug, PartialEq)]
enum LocStatus {
    /// GeoClue'dan konum bekleniyor.
    Pending,
    /// Otomatik bulundu.
    Auto,
    /// GeoClue kullanılamıyor (kapalı, izin yok, kurulu değil).
    Unavailable(String),
    /// Elle seçildi.
    Manual,
}

struct Runtime {
    district: Option<&'static District>,
    /// İlçe otomatik bulunmadı, saat diliminden tahmin edildi.
    guessed: bool,
    loc: LocStatus,
    times: Option<TimesCache>,
    fetch_error: Option<String>,
    next_fetch: DateTime<Utc>,
    backoff_min: i64,
    religious: HashMap<i32, Vec<ReligiousDay>>,
    religious_next_try: HashMap<i32, DateTime<Utc>>,
    persist: Persist,
    /// Bildirim kimliği → olay (düğmeler için).
    notes: HashMap<u32, Event>,
    /// Vakit başına son bildirim (ezan, ön uyarının yerini alır).
    note_by_vakit: HashMap<Vakit, u32>,
    paused_media: Vec<String>,
    /// Son yayınlanan "gün + şu anki vakit"; değişince durum yayınlanır.
    last_position: String,
}

pub struct Shared {
    cfg: RwLock<Config>,
    rt: Mutex<Runtime>,
    http: sakaleyn_core::http::Client,
    player: Player,
    wake: Notify,
    fetch_lock: Mutex<()>,
    conn: OnceLock<zbus::Connection>,
    cache_dir: PathBuf,
    state_path: PathBuf,
    location_task: std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
}

/// Düşürülünce görevi durdurur: dış görev iptal edilince iç görevler de kapanır.
struct AbortOnDrop(tokio::task::JoinHandle<()>);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

fn lang_of(cfg: &Config) -> Lang {
    Lang::resolve(&cfg.language)
}

fn hhmm(t: DateTime<Utc>, tz: Tz) -> String {
    t.with_timezone(&tz).format("%H:%M").to_string()
}

impl Runtime {
    fn tz(&self) -> Tz {
        self.district.map(|d| d.tz()).unwrap_or(chrono_tz::Europe::Istanbul)
    }

    fn local_date(&self, now: DateTime<Utc>) -> NaiveDate {
        now.with_timezone(&self.tz()).date_naive()
    }

    /// Günün tablosu: Diyanet önbelleği, yoksa yedek hesap.
    fn day(&self, date: NaiveDate) -> Option<(DayTimes, Source)> {
        if let Some(d) = self.times.as_ref().and_then(|c| c.get(date)) {
            return Some((d.clone(), Source::Diyanet));
        }
        let d = self.district?;
        let (lat, lon) = d.coords()?;
        Some((calc::day(date, lat, lon, d.tz()), Source::Calculated))
    }

    fn religious_for(&self, date: NaiveDate) -> &[ReligiousDay] {
        self.religious.get(&date.year()).map(|v| v.as_slice()).unwrap_or(&[])
    }

    fn events(&self, cfg: &Config, date: NaiveDate) -> Vec<Event> {
        let Some((day, _)) = self.day(date) else { return Vec::new() };
        schedule::build_day(&day, self.tz(), cfg, self.religious_for(date))
    }

    fn is_prayed(&self, date: NaiveDate, v: Vakit) -> bool {
        self.persist.prayed.contains(&format!("{date}:{}", v.key()))
    }

    fn set_district(&mut self, d: &'static District, cache_dir: &std::path::Path) {
        if self.district.map(|x| x.id) != Some(d.id) {
            self.times = TimesCache::load(cache_dir, d.id);
            self.fetch_error = None;
            self.next_fetch = DateTime::<Utc>::MIN_UTC;
            self.backoff_min = 10;
        }
        self.district = Some(d);
    }
}

impl Shared {
    fn conn(&self) -> Option<&zbus::Connection> {
        self.conn.get()
    }

    async fn save_persist(&self, rt: &Runtime) {
        if let Some(dir) = self.state_path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(b) = serde_json::to_vec_pretty(&rt.persist) {
            let tmp = self.state_path.with_extension("json.tmp");
            if std::fs::write(&tmp, b).is_ok() {
                let _ = std::fs::rename(tmp, &self.state_path);
            }
        }
    }

    async fn save_config(&self, cfg: &Config) {
        if let Err(e) = cfg.save() {
            log::warn!("cannot save config: {e}");
        }
    }

    // ---------------------------------------------------------------- durum

    pub async fn state(&self) -> Value {
        let cfg = self.cfg.read().await.clone();
        let rt = self.rt.lock().await;
        let lang = lang_of(&cfg);
        let now = Utc::now();
        let tz = rt.tz();
        let today = rt.local_date(now);
        let playing = self.player.playing().await;

        let day_json = |date: NaiveDate| -> Option<(Value, [DateTime<Utc>; 6], bool)> {
            let (d, source) = rt.day(date)?;
            let at = schedule::instants(&d, tz, &cfg);
            let mut times = serde_json::Map::new();
            let mut instants = serde_json::Map::new();
            for v in Vakit::ALL {
                times.insert(v.key().into(), json!(hhmm(at[v.index()], tz)));
                instants.insert(v.key().into(), json!(at[v.index()].to_rfc3339()));
            }
            let ramadan = cfg.ramadan.enabled && d.is_ramadan();
            Some((
                json!({
                    "date": date.to_string(),
                    "weekday": date.weekday().num_days_from_monday(),
                    "hijri": d.hijri,
                    "source": source,
                    "times": times,
                    "instants": instants,
                    "ramadan": ramadan,
                }),
                at,
                ramadan,
            ))
        };

        let t = day_json(today);
        let y = day_json(today.pred_opt().unwrap_or(today));
        let tm = day_json(today.succ_opt().unwrap_or(today));
        let (current, next) = match &t {
            Some((_, at, _)) => {
                let p = schedule::position(now, y.as_ref().map(|x| &x.1), at, tm.as_ref().map(|x| &x.1));
                (
                    p.current
                        .map(|(v, since)| json!({"vakit": v, "since": since.to_rfc3339()}))
                        .unwrap_or(Value::Null),
                    json!({"vakit": p.next.0, "at": p.next.1.to_rfc3339(), "local": hhmm(p.next.1, tz)}),
                )
            }
            None => (Value::Null, Value::Null),
        };
        let ramadan = t.as_ref().is_some_and(|x| x.2);
        // Ramazan: iftar (bugünkü akşam, geçmediyse) ve sahur sonu (sıradaki imsak).
        let (iftar, imsak_next) = match (&t, ramadan) {
            (Some((_, at, _)), true) => {
                let aksam = at[Vakit::Aksam.index()];
                let iftar = (aksam > now).then(|| aksam.to_rfc3339());
                let imsak = if at[0] > now {
                    Some(at[0])
                } else {
                    tm.as_ref().filter(|x| x.2).map(|x| x.1[0])
                };
                (iftar, imsak.map(|x| x.to_rfc3339()))
            }
            _ => (None, None),
        };

        let next_event = {
            let mut evs = rt.events(&cfg, today);
            if let Some(d) = today.succ_opt() {
                evs.extend(rt.events(&cfg, d));
            }
            evs.into_iter()
                .filter(|e| e.at > now && e.kind != EventKind::ReligiousDay)
                .map(|e| json!({"kind": e.kind, "vakit": e.vakit, "at": e.at.to_rfc3339(), "local": hhmm(e.at, tz)}))
                .next()
        };

        let religious_upcoming: Vec<Value> = {
            let mut v: Vec<&ReligiousDay> = rt.religious.values().flatten().filter(|r| r.date >= today).collect();
            v.sort_by_key(|r| r.date);
            v.into_iter()
                .take(3)
                .map(|r| json!({"date": r.date.to_string(), "name": r.name, "days": (r.date - today).num_days()}))
                .collect()
        };

        let prayed: Vec<&str> = Vakit::ALL
            .iter()
            .filter(|v| rt.is_prayed(today, **v))
            .map(|v| v.key())
            .collect();
        let alert_minutes: serde_json::Map<String, Value> = Vakit::ALL
            .iter()
            .map(|v| (v.key().to_string(), json!(cfg.alert_minutes(*v))))
            .collect();
        let adhan_on: serde_json::Map<String, Value> = Vakit::ADHAN
            .iter()
            .map(|v| (v.key().to_string(), json!(cfg.adhan_sound(*v).is_some())))
            .collect();
        let (loc_status, loc_message) = match &rt.loc {
            LocStatus::Pending => ("pending", None),
            LocStatus::Auto => ("auto", None),
            LocStatus::Unavailable(m) => ("unavailable", Some(m.clone())),
            LocStatus::Manual => ("manual", None),
        };

        json!({
            "schema": 1,
            "version": sakaleyn_core::VERSION,
            "lang": match lang { Lang::Tr => "tr", Lang::En => "en" },
            "now": now.to_rfc3339(),
            "district": rt.district.map(|d| json!({
                "id": d.id, "name": d.display(), "tz": d.tz,
                "country": d.country_name(),
            })),
            "location": {
                "mode": cfg.location.mode,
                "status": loc_status,
                "message": loc_message,
                "guessed": rt.guessed,
            },
            "fetched_at": rt.times.as_ref().map(|c| c.fetched_at.to_rfc3339()),
            "fetch_error": rt.fetch_error,
            "today": t.as_ref().map(|x| x.0.clone()),
            "tomorrow": tm.as_ref().map(|x| x.0.clone()),
            "current": current,
            "next": next,
            "next_event": next_event,
            "ramadan": ramadan,
            "iftar": iftar,
            "imsak": imsak_next,
            "friday": today.weekday() == chrono::Weekday::Fri,
            "muted_today": rt.persist.muted == Some(today),
            "enabled": cfg.enabled,
            "prayed": prayed,
            "playing": playing,
            "alert_minutes": alert_minutes,
            "adhan": adhan_on,
            "religious_upcoming": religious_upcoming,
            "player": Player::backend_name(),
        })
    }

    async fn emit_state(&self) {
        let Some(conn) = self.conn() else { return };
        let s = self.state().await.to_string();
        if let Ok(iface) = conn.object_server().interface::<_, Service>(OBJECT_PATH).await
            && let Err(e) = Service::state_changed(iface.signal_emitter(), &s).await
        {
            log::warn!("StateChanged failed: {e}");
        }
    }

    async fn emit_event(&self, kind: &str, vakit: &str) {
        let Some(conn) = self.conn() else { return };
        if let Ok(iface) = conn.object_server().interface::<_, Service>(OBJECT_PATH).await {
            let _ = Service::event(iface.signal_emitter(), kind, vakit).await;
        }
    }

    // ---------------------------------------------------------------- veri

    /// Diyanet tablosunu ve dini günleri gerekiyorsa indirir.
    async fn ensure_data(self: &Arc<Self>, force: bool) {
        let _guard = self.fetch_lock.lock().await;
        let now = Utc::now();
        let (district, today, need) = {
            let rt = self.rt.lock().await;
            let Some(d) = rt.district else { return };
            let today = rt.local_date(now);
            let need = force
                || (now >= rt.next_fetch && rt.times.as_ref().is_none_or(|c| c.needs_refresh(today, now)));
            (d, today, need)
        };
        let mut changed = false;
        if need {
            log::info!("fetching Diyanet times for district {}", district.id);
            match diyanet::fetch_district(&self.http, district.id).await {
                Ok(days) => {
                    let mut rt = self.rt.lock().await;
                    if rt.district.map(|d| d.id) == Some(district.id) {
                        let mut c = rt.times.take().unwrap_or_else(|| TimesCache::new(district.id));
                        c.merge(days, now, today);
                        if let Err(e) = c.save(&self.cache_dir) {
                            log::warn!("cannot write cache: {e}");
                        }
                        log::info!("got {} days up to {:?}", c.days.len(), c.covered_until());
                        rt.times = Some(c);
                        rt.fetch_error = None;
                        rt.backoff_min = 10;
                        rt.next_fetch = now + Duration::hours(6);
                        changed = true;
                    }
                }
                Err(e) => {
                    let mut rt = self.rt.lock().await;
                    log::warn!("Diyanet fetch failed: {e} (retry in {} min)", rt.backoff_min);
                    rt.fetch_error = Some(e.to_string());
                    rt.next_fetch = now + Duration::minutes(rt.backoff_min);
                    rt.backoff_min = (rt.backoff_min * 2).min(6 * 60);
                    changed = true;
                }
            }
        }
        // Dini günler: bu yıl, Aralık'ta gelecek yıl.
        let mut years = vec![today.year()];
        if today.month() == 12 {
            years.push(today.year() + 1);
        }
        for y in years {
            {
                let mut rt = self.rt.lock().await;
                if rt.religious.contains_key(&y) {
                    continue;
                }
                if let Some(v) = religious::load(&self.cache_dir, y) {
                    rt.religious.insert(y, v);
                    changed = true;
                    continue;
                }
                if rt.religious_next_try.get(&y).is_some_and(|t| *t > now) {
                    continue;
                }
                rt.religious_next_try.insert(y, now + Duration::minutes(30));
            }
            match religious::fetch(&self.http, y).await {
                Ok(v) => {
                    let _ = religious::save(&self.cache_dir, y, &v);
                    self.rt.lock().await.religious.insert(y, v);
                    changed = true;
                }
                Err(e) => log::info!("religious days {y}: {e}"),
            }
        }
        if changed {
            self.wake.notify_one();
            self.emit_state().await;
        }
    }

    // ---------------------------------------------------------------- konum

    /// İlçe değişti (otomatik konum, elle seçim ya da tahmin).
    async fn apply_district(self: &Arc<Self>, d: &'static District, how: LocStatus, announce: bool) {
        let changed = {
            let mut rt = self.rt.lock().await;
            let changed = rt.district.map(|x| x.id) != Some(d.id);
            rt.set_district(d, &self.cache_dir);
            rt.guessed = false;
            rt.loc = how;
            changed
        };
        {
            let mut cfg = self.cfg.write().await;
            if cfg.location.district_id != Some(d.id) {
                cfg.location.district_id = Some(d.id);
                self.save_config(&cfg).await;
            }
        }
        if changed && announce {
            let lang = lang_of(&*self.cfg.read().await);
            let (s, b) = match lang {
                Lang::Tr => ("Konum değişti".to_string(), format!("Vakitler artık {} için.", d.display())),
                Lang::En => ("Location changed".to_string(), format!("Prayer times are now for {}.", d.display())),
            };
            self.note(&s, &b, &[], false, None).await;
        }
        self.wake.notify_one();
        self.emit_state().await;
        let me = self.clone();
        tokio::spawn(async move { me.ensure_data(false).await });
    }

    async fn on_fix(self: &Arc<Self>, fix: geoclue::Fix) {
        if self.cfg.read().await.location.mode != LocationMode::Auto {
            return;
        }
        // Türkiye'de en yakın semt/köyün Diyanet ilçesi (Üsküdar → İstanbul), dışında en yakın şehir
        let Some((d, km, _place)) = districts::locate(fix.lat, fix.lon) else { return };
        log::info!("nearest district {} ({km:.1} km)", d.id);
        let had = self.rt.lock().await.district.is_some();
        let guessed = self.rt.lock().await.guessed;
        self.apply_district(d, LocStatus::Auto, had && !guessed).await;
    }

    /// İlçe hiç yoksa saat diliminden tahmin eder (ör. Türkiye → İstanbul). Tahmin
    /// kaydedilmez; GeoClue konum bulunca yerini alır.
    async fn ensure_guess(self: &Arc<Self>) {
        if self.rt.lock().await.district.is_some() {
            return;
        }
        let tz = system::system_timezone().unwrap_or_else(|| "Europe/Istanbul".into());
        if let Some(d) = districts::guess_from_timezone(&tz).or_else(|| districts::by_id(9541)) {
            {
                let mut rt = self.rt.lock().await;
                rt.set_district(d, &self.cache_dir);
                rt.guessed = true;
            }
            log::info!("guessed district {} from timezone {tz}", d.id);
            let me = self.clone();
            tokio::spawn(async move { me.ensure_data(false).await });
        }
    }

    /// GeoClue kullanılamıyor: daha önce bulunmuş ilçe varsa o kalır; yoksa tahmin
    /// kullanılır ve (bir kez) "ilçenizi seçin" bildirimi gelir.
    async fn on_location_error(self: &Arc<Self>, msg: String) {
        log::info!("geoclue unavailable: {msg}");
        self.rt.lock().await.loc = LocStatus::Unavailable(msg);
        self.ensure_guess().await;
        let (guessed, notice_shown, district) = {
            let rt = self.rt.lock().await;
            (rt.guessed, rt.persist.location_notice, rt.district)
        };
        if let (true, false, Some(d)) = (guessed, notice_shown, district) {
            let lang = lang_of(&*self.cfg.read().await);
            let (s, b) = match lang {
                Lang::Tr => (
                    "Konum bulunamadı".to_string(),
                    format!(
                        "Vakitler şimdilik {} için. Konum Hizmetlerini açın ya da Sakaleyn ayarlarından ilçenizi seçin.",
                        d.display()
                    ),
                ),
                Lang::En => (
                    "Location not found".to_string(),
                    format!(
                        "Showing times for {} for now. Turn on Location Services or choose your district in Sakaleyn settings.",
                        d.display()
                    ),
                ),
            };
            self.note(&s, &b, &[], false, None).await;
            let mut rt = self.rt.lock().await;
            rt.persist.location_notice = true;
            self.save_persist(&rt).await;
        }
        self.wake.notify_one();
        self.emit_state().await;
    }

    /// Otomatik konum görevini (yeniden) başlatır; elle modda durdurur.
    async fn restart_location(self: &Arc<Self>) {
        let auto = self.cfg.read().await.location.mode == LocationMode::Auto;
        if let Some(h) = self.location_task.lock().unwrap_or_else(|e| e.into_inner()).take() {
            h.abort();
        }
        if !auto {
            self.rt.lock().await.loc = LocStatus::Manual;
            return;
        }
        {
            let mut rt = self.rt.lock().await;
            if rt.loc != LocStatus::Auto {
                rt.loc = LocStatus::Pending;
            }
        }
        let me = self.clone();
        let handle = tokio::spawn(async move {
            loop {
                let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Result<geoclue::Fix, String>>();
                let tx2 = tx.clone();
                let watcher = AbortOnDrop(tokio::spawn(geoclue::watch(
                    move |f| drop(tx.send(Ok(f))),
                    move |e| drop(tx2.send(Err(e))),
                )));
                while let Some(msg) = rx.recv().await {
                    match msg {
                        Ok(fix) => me.on_fix(fix).await,
                        Err(e) => me.on_location_error(e).await,
                    }
                }
                drop(watcher);
                // GeoClue durdu ya da kullanılamıyor: 15 dk sonra yeniden dene.
                tokio::time::sleep(StdDuration::from_secs(15 * 60)).await;
            }
        });
        *self.location_task.lock().unwrap_or_else(|e| e.into_inner()) = Some(handle);
    }

    // ---------------------------------------------------------------- olaylar

    async fn note(&self, summary: &str, body: &str, actions: &[(&str, String)], resident: bool, replaces: Option<u32>) -> Option<u32> {
        let conn = self.conn()?;
        let n = Note {
            summary,
            body,
            actions,
            urgent: false,
            resident,
        };
        match notify::send(conn, &n, replaces.unwrap_or(0)).await {
            Ok(id) => Some(id),
            Err(e) => {
                log::warn!("notification failed: {e}");
                None
            }
        }
    }

    async fn fire(self: &Arc<Self>, e: &Event) {
        let cfg = self.cfg.read().await.clone();
        let lang = lang_of(&cfg);
        let now = Utc::now();
        let (tz, muted, prayed, replaces) = {
            let rt = self.rt.lock().await;
            let date = e.id.split(':').next().and_then(|d| d.parse().ok()).unwrap_or(rt.local_date(now));
            (
                rt.tz(),
                rt.persist.muted == Some(rt.local_date(now)),
                e.vakit.is_some_and(|v| rt.is_prayed(date, v)),
                e.vakit.and_then(|v| rt.note_by_vakit.get(&v).copied()),
            )
        };
        if e.kind == EventKind::EndReminder && prayed {
            return;
        }
        let conn = match self.conn() {
            Some(c) => c.clone(),
            None => return,
        };
        let dnd = system::do_not_disturb(&conn).await;
        let sound_allowed = !muted
            && match e.kind {
                EventKind::Adhan => !dnd || cfg.adhan.play_in_dnd,
                EventKind::Kerahat | EventKind::ReligiousDay => false,
                _ => !dnd || cfg.alerts.play_in_dnd,
            };
        let mut path = if sound_allowed { e.sound.as_deref().and_then(sounds::resolve) } else { None };
        if e.kind != EventKind::Adhan && self.player.playing().await.is_some_and(|p| p.kind == "adhan") {
            path = None; // ezanı uyarı sesiyle kesme
        }
        let remaining = ((e.target - now).num_seconds() as f64 / 60.0).round() as i64;
        let with_sound = path.is_some();
        let (summary, body) = i18n::event_text(lang, e, remaining, &hhmm(e.target, tz), with_sound);
        let snooze = cfg.alerts.snooze_minutes;
        let actions: Vec<(&str, String)> = match e.kind {
            EventKind::Adhan if with_sound => vec![
                ("stop", i18n::action_label(lang, "stop", snooze)),
                ("prayed", i18n::action_label(lang, "prayed", snooze)),
            ],
            EventKind::Adhan | EventKind::EndReminder => vec![("prayed", i18n::action_label(lang, "prayed", snooze))],
            EventKind::PreAlert | EventKind::Sahur | EventKind::Friday | EventKind::Sunrise => vec![
                ("snooze", i18n::action_label(lang, "snooze", snooze)),
                ("dismiss", i18n::action_label(lang, "dismiss", snooze)),
            ],
            _ => vec![],
        };
        log::info!("fire {} (sound: {})", e.id, path.as_ref().map(|p| p.display().to_string()).unwrap_or("-".into()));
        let resident = e.kind == EventKind::Adhan && with_sound;
        let id = self
            .note(&summary, &body, &actions, resident, if e.kind == EventKind::Adhan { replaces } else { None })
            .await;
        if let Some(id) = id {
            let mut rt = self.rt.lock().await;
            rt.notes.insert(id, e.clone());
            if let Some(v) = e.vakit {
                rt.note_by_vakit.insert(v, id);
            }
            if rt.notes.len() > 50 {
                let min = *rt.notes.keys().min().unwrap();
                rt.notes.remove(&min);
            }
        }
        if let Some(p) = path {
            let volume = if e.kind == EventKind::Adhan { cfg.adhan.volume } else { cfg.alerts.volume };
            if e.kind == EventKind::Adhan && cfg.adhan.pause_media {
                let paused = mpris::pause_all(&conn).await;
                self.rt.lock().await.paused_media = paused;
            }
            let info = Playing {
                kind: if e.kind == EventKind::Adhan { "adhan" } else { "alert" }.into(),
                vakit: e.vakit.map(|v| v.key().to_string()),
                sound: e.sound.clone().unwrap_or_default(),
            };
            let me = self.clone();
            let is_adhan = e.kind == EventKind::Adhan;
            let r = self
                .player
                .play(&p, volume, info, move || {
                    tokio::spawn(async move {
                        if is_adhan {
                            let names = std::mem::take(&mut me.rt.lock().await.paused_media);
                            if let Some(c) = me.conn() {
                                mpris::resume(c, &names).await;
                            }
                        }
                        me.emit_state().await;
                    });
                })
                .await;
            if let Err(err) = r {
                log::warn!("cannot play sound: {err}");
            }
        }
        self.emit_event(e.kind.key(), e.vakit.map(|v| v.key()).unwrap_or("")).await;
        self.emit_state().await;
    }

    async fn fire_missed(&self, e: &Event) {
        let Some(v) = e.vakit else { return };
        let (lang, tz) = (lang_of(&*self.cfg.read().await), self.rt.lock().await.tz());
        let (s, b) = i18n::missed_text(lang, v, &hhmm(e.target, tz));
        let actions = [("prayed", i18n::action_label(lang, "prayed", 5))];
        if let Some(id) = self.note(&s, &b, &actions, false, None).await {
            self.rt.lock().await.notes.insert(id, e.clone());
        }
    }

    /// Bir zamanlayıcı turu: vakti gelen olayları ateşler, bir sonraki uyanma anını döndürür.
    async fn tick(self: &Arc<Self>) -> DateTime<Utc> {
        let now = Utc::now();
        let cfg = self.cfg.read().await.clone();
        let (due, wake_points, position_key) = {
            let mut rt = self.rt.lock().await;
            let today = rt.local_date(now);
            let mut events = Vec::new();
            for d in [today.pred_opt(), Some(today), today.succ_opt()].into_iter().flatten() {
                events.extend(rt.events(&cfg, d));
            }
            events.extend(rt.persist.snoozed.iter().cloned());
            // Kayıt anahtarı ilçeyi de içerir: ilçe değişince (seyahat) yeni ilçenin
            // aynı adlı olayları "çalındı" sayılmasın.
            let district = rt.district.map(|d| d.id).unwrap_or(0);
            let key = |e: &Event| format!("{district}|{}", e.id);
            let mut due = Vec::new();
            for e in &events {
                if e.at <= now && !rt.persist.fired.contains_key(&key(e)) {
                    due.push(e.clone());
                }
            }
            for e in &due {
                rt.persist.fired.insert(key(e), now);
            }
            rt.persist.snoozed.retain(|e| e.at > now);
            let cutoff = now - Duration::days(3);
            rt.persist.fired.retain(|_, t| *t > cutoff);
            let keep_from = today - Duration::days(2);
            rt.persist.prayed.retain(|k| {
                k.split(':').next().and_then(|d| d.parse::<NaiveDate>().ok()).is_some_and(|d| d >= keep_from)
            });
            if !due.is_empty() {
                self.save_persist(&rt).await;
            }
            // Uyanma noktaları: sıradaki olaylar, vakit sınırları, yerel gece yarısı.
            let mut wake: Vec<DateTime<Utc>> = events.iter().map(|e| e.at).filter(|t| *t > now).collect();
            if let Some((d, _)) = rt.day(today) {
                wake.extend(schedule::instants(&d, rt.tz(), &cfg).into_iter().filter(|t| *t > now));
            }
            if let Some(midnight) = today.succ_opt().map(|d| schedule::to_utc(rt.tz(), d, chrono::NaiveTime::MIN)) {
                wake.push(midnight);
            }
            let pos = rt
                .day(today)
                .map(|(d, _)| {
                    let at = schedule::instants(&d, rt.tz(), &cfg);
                    at.iter().filter(|t| **t <= now).count()
                })
                .unwrap_or(0);
            (due, wake, format!("{today}:{pos}"))
        };

        for e in &due {
            let late = (now - e.at).num_seconds();
            if !cfg.enabled {
                log::debug!("disabled, skipped {}", e.id);
            } else if late <= GRACE {
                self.fire(e).await;
            } else if e.kind == EventKind::Adhan && late <= MISSED_WINDOW {
                self.fire_missed(e).await;
            } else {
                log::debug!("skipped stale {}", e.id);
            }
        }

        let changed = {
            let mut rt = self.rt.lock().await;
            let c = rt.last_position != position_key;
            rt.last_position = position_key;
            c
        };
        if changed {
            self.emit_state().await;
            let me = self.clone();
            tokio::spawn(async move { me.ensure_data(false).await });
        }

        let cap = now + Duration::seconds(MAX_SLEEP);
        wake_points.into_iter().filter(|t| *t > now).min().map_or(cap, |t| t.min(cap))
    }

    async fn scheduler(self: Arc<Self>) {
        loop {
            let next = self.tick().await;
            let ms = (next - Utc::now()).num_milliseconds().clamp(50, MAX_SLEEP * 1000);
            tokio::select! {
                _ = tokio::time::sleep(StdDuration::from_millis(ms as u64)) => {}
                _ = self.wake.notified() => {}
            }
        }
    }

    /// Bildirim düğmeleri.
    async fn actions(self: Arc<Self>) {
        loop {
            let run = async {
                let conn = self.conn().ok_or(zbus::Error::Failure("no connection".into()))?.clone();
                let proxy = notify::NotificationsProxy::new(&conn).await?;
                let mut s = proxy.receive_action_invoked().await?;
                while let Some(sig) = crate::dbus::next(&mut s).await {
                    let Ok(a) = sig.args() else { continue };
                    let ev = self.rt.lock().await.notes.get(a.id()).cloned();
                    let Some(ev) = ev else { continue };
                    self.on_action(&ev, a.action_key()).await;
                    notify::close(&conn, *a.id()).await;
                }
                Ok::<(), zbus::Error>(())
            };
            if let Err(e) = run.await {
                log::warn!("notification actions: {e}");
            }
            tokio::time::sleep(StdDuration::from_secs(5)).await;
        }
    }

    async fn on_action(self: &Arc<Self>, ev: &Event, key: &str) {
        log::info!("action {key} on {}", ev.id);
        match key {
            "stop" => {
                self.player.stop().await;
            }
            "prayed" => {
                if let Some(v) = ev.vakit {
                    let date: NaiveDate = ev
                        .id
                        .split(':')
                        .next()
                        .and_then(|d| d.parse().ok())
                        .unwrap_or_else(|| Utc::now().date_naive());
                    let mut rt = self.rt.lock().await;
                    rt.persist.prayed.insert(format!("{date}:{}", v.key()));
                    self.save_persist(&rt).await;
                }
                if self.player.playing().await.is_some_and(|p| p.vakit == ev.vakit.map(|v| v.key().to_string())) {
                    self.player.stop().await;
                }
            }
            "snooze" => {
                let minutes = self.cfg.read().await.alerts.snooze_minutes as i64;
                let mut e = ev.clone();
                e.at = Utc::now() + Duration::minutes(minutes);
                e.id = format!("{}:snooze:{}", ev.id, e.at.timestamp());
                self.player.stop().await;
                let mut rt = self.rt.lock().await;
                rt.persist.snoozed.push(e);
                self.save_persist(&rt).await;
                self.wake.notify_one();
            }
            _ => {}
        }
        self.emit_state().await;
    }
}

pub struct Service {
    shared: Arc<Shared>,
}

fn failed(e: impl std::fmt::Display) -> zbus::fdo::Error {
    zbus::fdo::Error::Failed(e.to_string())
}

#[zbus::interface(name = "io.github.ensignalp.Sakaleyn1")]
impl Service {
    async fn get_state(&self) -> String {
        self.shared.state().await.to_string()
    }

    async fn get_settings(&self) -> String {
        serde_json::to_string(&*self.shared.cfg.read().await).unwrap_or_else(|_| "{}".into())
    }

    async fn set_settings(&self, patch: &str) -> zbus::fdo::Result<()> {
        let patch: Value = serde_json::from_str(patch).map_err(failed)?;
        let (mode_changed, district_changed) = {
            let mut cfg = self.shared.cfg.write().await;
            let new = cfg.patched(&patch).map_err(failed)?;
            let mode_changed = new.location.mode != cfg.location.mode;
            let district_changed = new.location.district_id != cfg.location.district_id;
            *cfg = new;
            self.shared.save_config(&cfg).await;
            (mode_changed, district_changed)
        };
        if district_changed
            && let Some(d) = self.shared.cfg.read().await.location.district_id.and_then(districts::by_id)
        {
            let how = if self.shared.cfg.read().await.location.mode == LocationMode::Manual {
                LocStatus::Manual
            } else {
                LocStatus::Auto
            };
            self.shared.apply_district(d, how, false).await;
        }
        if mode_changed {
            self.shared.restart_location().await;
        }
        self.shared.wake.notify_one();
        self.shared.emit_state().await;
        Ok(())
    }

    async fn search_districts(&self, query: &str) -> String {
        let hits: Vec<Value> = districts::search(query, 30)
            .into_iter()
            .map(|h| json!({"id": h.district.id, "name": h.name, "via": h.via, "tz": h.district.tz}))
            .collect();
        Value::Array(hits).to_string()
    }

    async fn set_district(&self, id: u32) -> zbus::fdo::Result<()> {
        let d = districts::by_id(id).ok_or_else(|| failed(format!("unknown district {id}")))?;
        {
            let mut cfg = self.shared.cfg.write().await;
            cfg.location.mode = LocationMode::Manual;
            cfg.location.district_id = Some(id);
            self.shared.save_config(&cfg).await;
        }
        self.shared.restart_location().await;
        self.shared.apply_district(d, LocStatus::Manual, false).await;
        Ok(())
    }

    async fn use_auto_location(&self) {
        {
            let mut cfg = self.shared.cfg.write().await;
            cfg.location.mode = LocationMode::Auto;
            self.shared.save_config(&cfg).await;
        }
        self.shared.restart_location().await;
        self.shared.emit_state().await;
    }

    async fn refresh_times(&self) {
        let s = self.shared.clone();
        tokio::spawn(async move { s.ensure_data(true).await });
    }

    async fn stop_audio(&self) {
        if self.shared.player.stop().await {
            self.shared.emit_state().await;
        }
    }

    async fn mute_today(&self, muted: bool) {
        {
            let mut rt = self.shared.rt.lock().await;
            let today = rt.local_date(Utc::now());
            rt.persist.muted = muted.then_some(today);
            self.shared.save_persist(&rt).await;
        }
        if muted {
            self.shared.player.stop().await;
        }
        self.shared.emit_state().await;
    }

    async fn prayed(&self, vakit: &str) -> zbus::fdo::Result<()> {
        let v = Vakit::from_key(vakit).ok_or_else(|| failed(format!("unknown vakit {vakit}")))?;
        {
            let mut rt = self.shared.rt.lock().await;
            let today = rt.local_date(Utc::now());
            let key = format!("{today}:{}", v.key());
            if !rt.persist.prayed.remove(&key) {
                rt.persist.prayed.insert(key);
            }
            self.shared.save_persist(&rt).await;
        }
        self.shared.emit_state().await;
        Ok(())
    }

    async fn preview_sound(&self, id: &str) -> zbus::fdo::Result<()> {
        if self
            .shared
            .player
            .playing()
            .await
            .is_some_and(|p| p.kind == "preview" && p.sound == id)
        {
            self.shared.player.stop().await;
            self.shared.emit_state().await;
            return Ok(());
        }
        let lang = lang_of(&*self.shared.cfg.read().await);
        let info = sounds::catalog(lang)
            .into_iter()
            .find(|s| s.id == id)
            .ok_or_else(|| failed(format!("unknown sound {id}")))?;
        let cfg = self.shared.cfg.read().await.clone();
        let volume = if info.kind == "alert" { cfg.alerts.volume } else { cfg.adhan.volume };
        let me = self.shared.clone();
        self.shared
            .player
            .play(
                &info.path,
                volume,
                Playing {
                    kind: "preview".into(),
                    vakit: None,
                    sound: id.into(),
                },
                move || {
                    tokio::spawn(async move { me.emit_state().await });
                },
            )
            .await
            .map_err(failed)?;
        self.shared.emit_state().await;
        Ok(())
    }

    /// Olayı şimdi dener: `kind` = pre_alert | adhan | sahur | friday | …, `vakit` = ogle …
    /// Bildirim ve ses gerçek olaydaki gibidir (ayarlara uyar).
    async fn test_event(&self, kind: &str, vakit: &str) -> zbus::fdo::Result<()> {
        let v = Vakit::from_key(vakit).ok_or_else(|| failed(format!("unknown vakit {vakit}")))?;
        let kind: EventKind = serde_json::from_value(json!(kind)).map_err(failed)?;
        let cfg = self.shared.cfg.read().await.clone();
        let now = Utc::now();
        let minutes = match kind {
            EventKind::PreAlert => cfg.alert_minutes(v).unwrap_or(cfg.alerts.minutes) as i64,
            EventKind::Friday => cfg.friday.minutes as i64,
            EventKind::Sahur => cfg.ramadan.sahur_minutes as i64,
            EventKind::Sunrise => cfg.sunrise.minutes as i64,
            EventKind::EndReminder => cfg.end_reminder.minutes as i64,
            _ => 0,
        };
        let sound = match kind {
            EventKind::Adhan => cfg.adhan_sound(v),
            EventKind::Sahur => Some(cfg.ramadan.sahur_sound.clone()),
            EventKind::Kerahat | EventKind::ReligiousDay => None,
            _ => Some(cfg.alerts.sound.clone()),
        };
        let e = Event {
            id: format!("test:{}:{}", kind.key(), now.timestamp_millis()),
            at: now,
            kind,
            vakit: Some(v),
            target: now + Duration::minutes(minutes),
            sound,
            label: (kind == EventKind::ReligiousDay).then(|| "Test Kandili".to_string()),
            ramadan: false,
            friday: false,
        };
        self.shared.fire(&e).await;
        Ok(())
    }

    async fn list_sounds(&self) -> String {
        let lang = lang_of(&*self.shared.cfg.read().await);
        serde_json::to_string(&sounds::catalog(lang)).unwrap_or_else(|_| "[]".into())
    }

    #[zbus(property)]
    async fn version(&self) -> String {
        sakaleyn_core::VERSION.to_string()
    }

    #[zbus(signal)]
    async fn state_changed(emitter: &SignalEmitter<'_>, state: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn event(emitter: &SignalEmitter<'_>, kind: &str, vakit: &str) -> zbus::Result<()>;
}

pub async fn run() -> Result<(), String> {
    let (cfg, warning) = Config::load();
    if let Some(w) = warning {
        log::warn!("config file ignored, using defaults: {w}");
    }
    let cache_dir = sakaleyn_core::paths::cache_dir();
    let state_path = sakaleyn_core::paths::state_dir().join("state.json");
    let persist: Persist = std::fs::read(&state_path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let district = cfg.location.district_id.and_then(districts::by_id);
    let loc = match cfg.location.mode {
        LocationMode::Manual => LocStatus::Manual,
        LocationMode::Auto => LocStatus::Pending,
    };
    log::info!(
        "sakaleyn {} starting: district {:?}, location {:?}, player {:?}",
        sakaleyn_core::VERSION,
        district.map(|d| d.id),
        cfg.location.mode,
        Player::backend_name()
    );
    let mut rt = Runtime {
        district: None,
        guessed: false,
        loc,
        times: None,
        fetch_error: None,
        next_fetch: DateTime::<Utc>::MIN_UTC,
        backoff_min: 10,
        religious: HashMap::new(),
        religious_next_try: HashMap::new(),
        persist,
        notes: HashMap::new(),
        note_by_vakit: HashMap::new(),
        paused_media: Vec::new(),
        last_position: String::new(),
    };
    if let Some(d) = district {
        rt.set_district(d, &cache_dir);
    }
    let shared = Arc::new(Shared {
        cfg: RwLock::new(cfg),
        rt: Mutex::new(rt),
        http: sakaleyn_core::http::build_client(),
        player: Player::new(),
        wake: Notify::new(),
        fetch_lock: Mutex::new(()),
        conn: OnceLock::new(),
        cache_dir,
        state_path,
        location_task: std::sync::Mutex::new(None),
    });

    let conn = zbus::connection::Builder::session()
        .name(BUS_NAME)
        .serve_at(OBJECT_PATH, Service { shared: shared.clone() })
        .build()
        .await
        .map_err(|e| format!("cannot own {BUS_NAME} on the session bus (already running?): {e}"))?;
    let _ = shared.conn.set(conn);
    log::info!("listening on {BUS_NAME}");

    if shared.cfg.read().await.location.mode == LocationMode::Auto {
        shared.ensure_guess().await;
    }
    shared.restart_location().await;
    tokio::spawn(shared.clone().scheduler());
    tokio::spawn(shared.clone().actions());
    {
        let s = shared.clone();
        tokio::spawn(async move { s.ensure_data(false).await });
    }
    {
        let s = shared.clone();
        tokio::spawn(system::watch_resume(move || {
            let s = s.clone();
            tokio::spawn(async move {
                s.wake.notify_one();
                // ağ uyanınca kendine gelsin diye biraz bekle
                tokio::time::sleep(StdDuration::from_secs(10)).await;
                s.ensure_data(false).await;
            });
        }));
    }

    use tokio::signal::unix::{SignalKind, signal};
    let mut term = signal(SignalKind::terminate()).map_err(|e| e.to_string())?;
    let mut int = signal(SignalKind::interrupt()).map_err(|e| e.to_string())?;
    tokio::select! {
        _ = term.recv() => {}
        _ = int.recv() => {}
    }
    log::info!("stopping");
    shared.player.stop().await;
    let names = std::mem::take(&mut shared.rt.lock().await.paused_media);
    if let Some(c) = shared.conn() {
        mpris::resume(c, &names).await;
    }
    Ok(())
}

//! Sakaleyn: Linux için ezan vakti uygulaması.
//!
//! `sakaleyn daemon` arka plan servisidir (systemd kullanıcı servisi ya da D-Bus
//! ile başlar). Diğer komutlar servis olmadan da çalışır; servis açıksa ayar
//! değişikliklerini ona iletir.

mod daemon;
mod dbus;
mod geoclue;
mod i18n;
mod mpris;
mod notify;
mod sounds;
mod system;
#[cfg(feature = "tray")]
mod tray;

use std::time::Duration;

use chrono::{NaiveDate, Utc};
use clap::{Parser, Subcommand};
use sakaleyn_core::cache::TimesCache;
use sakaleyn_core::config::{Config, LocationMode};
use sakaleyn_core::districts::{self, District};
use sakaleyn_core::model::{Lang, Source, Vakit};
use sakaleyn_core::{calc, diyanet, schedule};

#[derive(Parser)]
#[command(name = "sakaleyn", version, about = "Ezan vakitleri, vakit öncesi uyarı ve ezan (Diyanet verisi)")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Bugünün (ya da verilen günün) vakitleri.
    Today {
        /// YYYY-AA-GG
        #[arg(long)]
        date: Option<NaiveDate>,
        /// İlçe kimliği (varsayılan: ayarlardaki).
        #[arg(long)]
        district: Option<u32>,
        #[arg(long)]
        json: bool,
    },
    /// Sıradaki vakit ve kalan süre.
    Next,
    /// Servisin anlık durumu (JSON).
    Status,
    /// GeoClue ile konumu bul ve en yakın Diyanet ilçesini göster.
    Where,
    /// İlçe arama / seçme.
    City {
        #[command(subcommand)]
        cmd: CityCmd,
    },
    /// Sesleri listele.
    Sounds,
    /// Bir sesi çal: ses kimliği (`kabe`, `nur`) ya da vakit (`ogle` → o vaktin ezanı).
    TestSound { id: String },
    /// Servis üzerinden bir olayı şimdi dene: `sakaleyn test-event pre_alert ogle`.
    TestEvent { kind: String, vakit: String },
    /// Servisin çaldığı sesi durdur.
    Stop,
    /// Arka plan servisi.
    Daemon,
    /// Tepsi simgesi (KDE, XFCE, Cinnamon, MATE…).
    #[cfg(feature = "tray")]
    Tray,
    /// Ayarlar penceresini aç.
    Settings,
}

#[derive(Subcommand)]
enum CityCmd {
    /// Ada göre ara: `sakaleyn city search kadıköy`.
    Search { query: Vec<String> },
    /// Elle seç (otomatik konum kapanır).
    Set { id: u32 },
    /// Otomatik konuma dön.
    Auto,
}

fn main() {
    // `sakaleyn city search x | head` gibi kullanımda panik yerine sessizce çık.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let cli = Cli::parse();
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().expect("tokio runtime");
    if let Err(e) = rt.block_on(run(cli.cmd)) {
        eprintln!("sakaleyn: {e}");
        std::process::exit(1);
    }
}

fn lang() -> Lang {
    Lang::resolve(&Config::load().0.language)
}

/// Ayarlardaki ilçe; yoksa saat diliminden tahmin.
fn configured_district(cfg: &Config, explicit: Option<u32>) -> Result<&'static District, String> {
    if let Some(id) = explicit.or(cfg.location.district_id) {
        return districts::by_id(id).ok_or_else(|| format!("unknown district {id}"));
    }
    let tz = system::system_timezone().unwrap_or_else(|| "Europe/Istanbul".into());
    districts::guess_from_timezone(&tz).ok_or_else(|| "no district configured: run `sakaleyn city search <ad>`".into())
}

/// Önbellek → (gerekirse) Diyanet → yedek hesap.
async fn day_for(d: &'static District, date: NaiveDate) -> (sakaleyn_core::model::DayTimes, Source) {
    let dir = sakaleyn_core::paths::cache_dir();
    let mut cache = TimesCache::load(&dir, d.id);
    let now = Utc::now();
    if cache.as_ref().and_then(|c| c.get(date)).is_none() {
        let client = sakaleyn_core::http::build_client();
        match diyanet::fetch_district(&client, d.id).await {
            Ok(days) => {
                let mut c = cache.take().unwrap_or_else(|| TimesCache::new(d.id));
                c.merge(days, now, now.with_timezone(&d.tz()).date_naive());
                let _ = c.save(&dir);
                cache = Some(c);
            }
            Err(e) => eprintln!("Diyanet: {e} (hesaplanan vakitler gösteriliyor)"),
        }
    }
    if let Some(day) = cache.as_ref().and_then(|c| c.get(date)) {
        return (day.clone(), Source::Diyanet);
    }
    let (lat, lon) = d.coords().unwrap_or((41.0, 29.0));
    (calc::day(date, lat, lon, d.tz()), Source::Calculated)
}

async fn run(cmd: Cmd) -> Result<(), String> {
    match cmd {
        Cmd::Today { date, district, json } => {
            let cfg = Config::load().0;
            let d = configured_district(&cfg, district)?;
            let date = date.unwrap_or_else(|| Utc::now().with_timezone(&d.tz()).date_naive());
            let (day, source) = day_for(d, date).await;
            let at = schedule::instants(&day, d.tz(), &cfg);
            let lang = Lang::resolve(&cfg.language);
            if json {
                let times: serde_json::Map<_, _> = Vakit::ALL
                    .iter()
                    .map(|v| {
                        (
                            v.key().to_string(),
                            serde_json::json!(at[v.index()].with_timezone(&d.tz()).format("%H:%M").to_string()),
                        )
                    })
                    .collect();
                println!(
                    "{}",
                    serde_json::json!({"district": d.display(), "id": d.id, "date": date.to_string(),
                        "hijri": day.hijri, "source": source, "times": times})
                );
            } else {
                println!("{} · {} {}", d.display(), date.format("%d.%m.%Y"), day.hijri);
                for v in Vakit::ALL {
                    println!("  {:<8} {}", v.name(lang), at[v.index()].with_timezone(&d.tz()).format("%H:%M"));
                }
                if source == Source::Calculated {
                    println!("  ({})", i18n::t(lang, "tahmini: Diyanet'e ulaşılamadı", "estimated: Diyanet unreachable"));
                }
            }
        }
        Cmd::Next => {
            let cfg = Config::load().0;
            let d = configured_district(&cfg, None)?;
            let tz = d.tz();
            let now = Utc::now();
            let today = now.with_timezone(&tz).date_naive();
            let t = schedule::instants(&day_for(d, today).await.0, tz, &cfg);
            let tm = match today.succ_opt() {
                Some(n) => Some(schedule::instants(&day_for(d, n).await.0, tz, &cfg)),
                None => None,
            };
            let p = schedule::position(now, None, &t, tm.as_ref());
            let lang = Lang::resolve(&cfg.language);
            let mins = ((p.next.1 - now).num_seconds() + 59) / 60;
            println!(
                "{} {} ({})",
                p.next.0.name(lang),
                p.next.1.with_timezone(&tz).format("%H:%M"),
                i18n::duration(lang, mins)
            );
        }
        Cmd::Status => {
            let p = dbus::proxy().await?;
            let s = p.get_state().await.map_err(|e| format!("service not running? {e}"))?;
            let v: serde_json::Value = serde_json::from_str(&s).map_err(|e| e.to_string())?;
            println!("{}", serde_json::to_string_pretty(&v).unwrap_or(s));
        }
        Cmd::Where => {
            let fix = geoclue::once(Duration::from_secs(30)).await?;
            let (d, km, place) = districts::locate(fix.lat, fix.lon).ok_or("district database empty")?;
            let at = place.map(|p| format!("{p} → ")).unwrap_or_default();
            println!("{at}{} (id {}, {:.1} km, doğruluk ~{:.0} m)", d.display(), d.id, km, fix.accuracy_m);
        }
        Cmd::City { cmd } => match cmd {
            CityCmd::Search { query } => {
                let q = query.join(" ");
                for h in districts::search(&q, 20) {
                    match h.via {
                        Some(via) => println!("{:>6}  {}  ({via} vakitleri)", h.district.id, h.name),
                        None => println!("{:>6}  {}", h.district.id, h.name),
                    }
                }
            }
            CityCmd::Set { id } => {
                let d = districts::by_id(id).ok_or_else(|| format!("unknown district {id}"))?;
                match dbus::proxy().await {
                    Ok(p) if p.set_district(id).await.is_ok() => {}
                    _ => {
                        let mut cfg = Config::load().0;
                        cfg.location.mode = LocationMode::Manual;
                        cfg.location.district_id = Some(id);
                        cfg.save().map_err(|e| e.to_string())?;
                    }
                }
                println!("{}", d.display());
            }
            CityCmd::Auto => match dbus::proxy().await {
                Ok(p) if p.use_auto_location().await.is_ok() => {}
                _ => {
                    let mut cfg = Config::load().0;
                    cfg.location.mode = LocationMode::Auto;
                    cfg.save().map_err(|e| e.to_string())?;
                }
            },
        },
        Cmd::Sounds => {
            for s in sounds::catalog(lang()) {
                println!("{:<24} {:<6} {}", s.id, s.kind, s.title);
            }
        }
        Cmd::TestSound { id } => {
            let cfg = Config::load().0;
            let (sound, volume) = match Vakit::from_key(&id) {
                Some(v) => (
                    cfg.adhan_sound(v).ok_or_else(|| format!("{id}: bu vakitte ezan kapalı"))?,
                    cfg.adhan.volume,
                ),
                None => {
                    let alert = sounds::catalog(Lang::Tr).iter().any(|s| s.id == id && s.kind == "alert");
                    (id.clone(), if alert { cfg.alerts.volume } else { cfg.adhan.volume })
                }
            };
            let path = sounds::resolve(&sound).ok_or_else(|| format!("unknown sound {sound} (see `sakaleyn sounds`)"))?;
            let player = sounds::Player::new();
            let (tx, rx) = tokio::sync::oneshot::channel();
            let info = sounds::Playing {
                kind: "preview".into(),
                vakit: None,
                sound: sound.clone(),
            };
            player.play(&path, volume, info, move || {
                let _ = tx.send(());
            })
            .await?;
            println!("{sound} çalıyor… (Ctrl+C durdurur)");
            tokio::select! {
                _ = rx => {}
                _ = tokio::signal::ctrl_c() => { player.stop().await; }
            }
        }
        Cmd::TestEvent { kind, vakit } => {
            dbus::proxy().await?.test_event(&kind, &vakit).await.map_err(|e| e.to_string())?;
        }
        Cmd::Stop => {
            dbus::proxy().await?.stop_audio().await.map_err(|e| e.to_string())?;
        }
        Cmd::Daemon => daemon::run().await?,
        #[cfg(feature = "tray")]
        Cmd::Tray => tray::run().await?,
        Cmd::Settings => open_settings()?,
    }
    Ok(())
}

/// Ayarlar: GNOME'da eklenti tercihleri, diğer masaüstlerinde aynı arayüzün
/// bağımsız GJS uygulaması (`…/share/sakaleyn/settings/main.js`).
pub fn open_settings() -> Result<(), String> {
    let uuid = "sakaleyn@ensignalp";
    let gnome = std::env::var("XDG_CURRENT_DESKTOP").is_ok_and(|d| d.to_lowercase().contains("gnome"));
    if gnome
        && std::process::Command::new("gnome-extensions")
            .args(["prefs", uuid])
            .status()
            .is_ok_and(|s| s.success())
    {
        return Ok(());
    }
    let mut dirs = vec![sakaleyn_core::paths::home_dir().join(".local/share")];
    for d in std::env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| "/usr/local/share:/usr/share".into())
        .split(':')
    {
        dirs.push(d.into());
    }
    let main = dirs
        .into_iter()
        .map(|d| d.join("sakaleyn/settings/main.js"))
        .find(|p| p.exists())
        .ok_or("settings app not installed (…/share/sakaleyn/settings/main.js)")?;
    std::process::Command::new("gjs")
        .arg("-m")
        .arg(main)
        .spawn()
        .map_err(|e| format!("gjs: {e}"))?;
    Ok(())
}

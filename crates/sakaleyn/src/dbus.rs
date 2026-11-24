//! D-Bus sözleşmesi (servis tarafı `daemon.rs`, istemciler: CLI, tepsi, GNOME eklentisi).
//!
//! servis:   io.github.ensignalp.Sakaleyn
//! nesne:    /io/github/ensignalp/Sakaleyn
//! arayüz:   io.github.ensignalp.Sakaleyn1
//!
//!   GetState() -> s               anlık durum (JSON, bkz. daemon::State)
//!   GetSettings() -> s            config.toml'un JSON hâli
//!   SetSettings(s patch)          JSON yaması; kaydedilir, plan yeniden kurulur
//!   SearchDistricts(s q) -> s     [{id, name, via, tz}]; via: semtin kullandığı Diyanet ilçesi
//!   SetDistrict(u id)             elle konum
//!   UseAutoLocation()             GeoClue ile otomatik konum
//!   RefreshTimes()                Diyanet'ten şimdi indir
//!   StopAudio()                   çalan sesi durdur
//!   MuteToday(b)                  bugün yalnızca bildirim
//!   Prayed(s vakit)               "kıldım": o vaktin kalan hatırlatmaları kapanır
//!   PreviewSound(s id)            sesi dene (yeniden çağrılırsa durur)
//!   ListSounds() -> s             [{id, kind, title, path}]
//!   TestEvent(s kind, s vakit)    olayı şimdi dene (bildirim + ses)
//!   signal StateChanged(s)        durum değişince
//!   signal Event(s kind, s vakit) ön uyarı / ezan / … ateşlenince
//!   property Version (s)

use std::time::Duration;

pub const BUS_NAME: &str = "io.github.ensignalp.Sakaleyn";
pub const OBJECT_PATH: &str = "/io/github/ensignalp/Sakaleyn";

#[zbus::proxy(
    interface = "io.github.ensignalp.Sakaleyn1",
    default_service = "io.github.ensignalp.Sakaleyn",
    default_path = "/io/github/ensignalp/Sakaleyn"
)]
pub trait Sakaleyn {
    fn get_state(&self) -> zbus::Result<String>;
    fn get_settings(&self) -> zbus::Result<String>;
    fn set_settings(&self, patch: &str) -> zbus::Result<()>;
    fn search_districts(&self, query: &str) -> zbus::Result<String>;
    fn set_district(&self, id: u32) -> zbus::Result<()>;
    fn use_auto_location(&self) -> zbus::Result<()>;
    fn refresh_times(&self) -> zbus::Result<()>;
    fn stop_audio(&self) -> zbus::Result<()>;
    fn mute_today(&self, muted: bool) -> zbus::Result<()>;
    fn prayed(&self, vakit: &str) -> zbus::Result<()>;
    fn preview_sound(&self, id: &str) -> zbus::Result<()>;
    fn list_sounds(&self) -> zbus::Result<String>;
    fn test_event(&self, kind: &str, vakit: &str) -> zbus::Result<()>;
    #[zbus(property)]
    fn version(&self) -> zbus::Result<String>;
    #[zbus(signal)]
    fn state_changed(&self, state: String) -> zbus::Result<()>;
}

pub async fn proxy() -> Result<SakaleynProxy<'static>, String> {
    let conn = zbus::Connection::session().await.map_err(|e| e.to_string())?;
    SakaleynProxy::new(&conn).await.map_err(|e| e.to_string())
}

/// Servisin her yeni durumunda `on_state` çağrılır (ilk değer dahil). Servis
/// yeniden başlarsa bağlantı yeniden kurulur; asla dönmez.
pub async fn watch(mut on_state: impl FnMut(serde_json::Value)) -> ! {
    loop {
        let run = async {
            let conn = zbus::Connection::session().await?;
            let proxy = SakaleynProxy::new(&conn).await?;
            let mut updates = proxy.receive_state_changed().await?;
            if let Ok(v) = serde_json::from_str(&proxy.get_state().await?) {
                on_state(v);
            }
            while let Some(sig) = next(&mut updates).await {
                if let Ok(args) = sig.args()
                    && let Ok(v) = serde_json::from_str(args.state())
                {
                    on_state(v);
                }
            }
            Ok::<(), zbus::Error>(())
        };
        if let Err(e) = run.await {
            log::debug!("watch: {e}");
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

/// `Stream::next` (futures-util'e bağımlı olmadan).
pub async fn next<S: zbus::export::futures_core::Stream + Unpin>(s: &mut S) -> Option<S::Item> {
    use std::pin::Pin;
    std::future::poll_fn(|cx| Pin::new(&mut *s).poll_next(cx)).await
}

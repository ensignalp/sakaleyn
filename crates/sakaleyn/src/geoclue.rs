//! Otomatik konum: GeoClue2 (sistem D-Bus'ı). GNOME'da Ayarlar → Gizlilik →
//! Konum Hizmetleri açık olmalı; ilk kullanımda GNOME izin sorabilir. Konum
//! yalnızca en yakın Diyanet ilçesini bulmak için kullanılır, hiçbir yere gönderilmez.

use std::time::Duration;

#[zbus::proxy(
    interface = "org.freedesktop.GeoClue2.Manager",
    default_service = "org.freedesktop.GeoClue2",
    default_path = "/org/freedesktop/GeoClue2/Manager"
)]
trait Manager {
    fn get_client(&self) -> zbus::Result<zbus::OwnedObjectPath>;
}

#[zbus::proxy(interface = "org.freedesktop.GeoClue2.Client", default_service = "org.freedesktop.GeoClue2")]
trait Client {
    fn start(&self) -> zbus::Result<()>;
    fn stop(&self) -> zbus::Result<()>;
    #[zbus(property)]
    fn set_desktop_id(&self, id: &str) -> zbus::Result<()>;
    #[zbus(property)]
    fn set_distance_threshold(&self, meters: u32) -> zbus::Result<()>;
    #[zbus(property)]
    fn set_requested_accuracy_level(&self, level: u32) -> zbus::Result<()>;
    #[zbus(signal)]
    fn location_updated(
        &self,
        old: zbus::OwnedObjectPath,
        new: zbus::OwnedObjectPath,
    ) -> zbus::Result<()>;
}

#[zbus::proxy(interface = "org.freedesktop.GeoClue2.Location", default_service = "org.freedesktop.GeoClue2")]
trait Location {
    #[zbus(property)]
    fn latitude(&self) -> zbus::Result<f64>;
    #[zbus(property)]
    fn longitude(&self) -> zbus::Result<f64>;
    #[zbus(property)]
    fn accuracy(&self) -> zbus::Result<f64>;
}

/// GeoClue istemcisinin kurulup başlaması için en uzun bekleme.
const SETUP_TIMEOUT: Duration = Duration::from_secs(120);

/// GeoClue doğruluk düzeyi: sokak (~1 km). İlçe bulmak için yeterli.
const ACCURACY_STREET: u32 = 6;

#[derive(Clone, Copy, Debug)]
pub struct Fix {
    pub lat: f64,
    pub lon: f64,
    pub accuracy_m: f64,
}

/// Konum güncellemelerini dinler (5 km'den fazla değişince). `on_fix` her yeni
/// konumda, `on_error` GeoClue kullanılamazsa çağrılır. GeoClue durursa çıkar;
/// çağıran yeniden dener.
pub async fn watch(on_fix: impl Fn(Fix) + Send + Sync, on_error: impl Fn(String)) {
    let run = async {
        let conn = zbus::Connection::system().await?;
        // GeoClue takılabiliyor (aracıyı/ağ konumunu beklerken GetClient bile dönmüyor);
        // kurulum adımları sınırlı süre beklenir, aşılırsa çağıran sonra yeniden dener.
        // Start, GNOME'un izin penceresini de bekleyebildiği için süre cömert.
        let setup = async {
            let manager = ManagerProxy::new(&conn).await?;
            let path = manager.get_client().await?;
            let client = ClientProxy::builder(&conn).path(path).build().await?;
            client.set_desktop_id("sakaleyn").await?;
            client.set_distance_threshold(5000).await?;
            client.set_requested_accuracy_level(ACCURACY_STREET).await?;
            let updates = client.receive_location_updated().await?;
            client.start().await?;
            Ok::<_, zbus::Error>((client, updates))
        };
        let (_client, mut updates) = tokio::time::timeout(SETUP_TIMEOUT, setup)
            .await
            .map_err(|_| zbus::Error::Failure(format!("GeoClue did not respond within {}s", SETUP_TIMEOUT.as_secs())))??;
        log::info!("geoclue started");
        while let Some(sig) = crate::dbus::next(&mut updates).await {
            let Ok(args) = sig.args() else { continue };
            let loc = LocationProxy::builder(&conn).path(args.new().clone()).build().await?;
            let fix = Fix {
                lat: loc.latitude().await?,
                lon: loc.longitude().await?,
                accuracy_m: loc.accuracy().await.unwrap_or(f64::NAN),
            };
            log::info!("location fix (accuracy {:.0} m)", fix.accuracy_m);
            on_fix(fix);
        }
        Ok::<(), zbus::Error>(())
    };
    if let Err(e) = run.await {
        on_error(e.to_string());
    }
}

/// İlk konumu en fazla `timeout` bekler (CLI `sakaleyn where` için).
pub async fn once(timeout: Duration) -> Result<Fix, String> {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Result<Fix, String>>();
    let tx2 = tx.clone();
    let task = tokio::spawn(async move {
        watch(move |f| drop(tx.send(Ok(f))), move |e| drop(tx2.send(Err(e)))).await;
    });
    let r = tokio::time::timeout(timeout, rx.recv()).await;
    task.abort();
    match r {
        Ok(Some(x)) => x,
        Ok(None) => Err("geoclue stopped".into()),
        Err(_) => Err(format!("no location within {}s", timeout.as_secs())),
    }
}

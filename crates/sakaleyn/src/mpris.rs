//! Ezan sırasında çalan medya oynatıcılarını (Spotify, tarayıcı, VLC…) duraklatır,
//! ezan bitince yalnızca duraklattıklarını devam ettirir (MPRIS 2).

const PREFIX: &str = "org.mpris.MediaPlayer2.";

#[zbus::proxy(interface = "org.mpris.MediaPlayer2.Player", default_path = "/org/mpris/MediaPlayer2")]
trait Player {
    fn pause(&self) -> zbus::Result<()>;
    fn play(&self) -> zbus::Result<()>;
    #[zbus(property)]
    fn playback_status(&self) -> zbus::Result<String>;
}

async fn player<'a>(conn: &'a zbus::Connection, name: &'a str) -> zbus::Result<PlayerProxy<'a>> {
    PlayerProxy::builder(conn)
        .destination(name.to_string())
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await
}

/// Çalan oynatıcıları duraklatır; duraklatılanların bus adlarını döndürür.
pub async fn pause_all(conn: &zbus::Connection) -> Vec<String> {
    let mut paused = Vec::new();
    let Ok(dbus) = zbus::fdo::DBusProxy::new(conn).await else { return paused };
    let Ok(names) = dbus.list_names().await else { return paused };
    for n in names.iter().map(|n| n.to_string()).filter(|n| n.starts_with(PREFIX)) {
        let Ok(p) = player(conn, &n).await else { continue };
        if p.playback_status().await.is_ok_and(|s| s == "Playing") && p.pause().await.is_ok() {
            log::info!("paused {n}");
            paused.push(n);
        }
    }
    paused
}

pub async fn resume(conn: &zbus::Connection, names: &[String]) {
    for n in names {
        if let Ok(p) = player(conn, n).await
            && p.playback_status().await.is_ok_and(|s| s == "Paused")
        {
            let _ = p.play().await;
            log::info!("resumed {n}");
        }
    }
}

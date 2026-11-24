//! Sistem sinyalleri: uyku/uyanma (logind) ve "Rahatsız Etmeyin" durumu.

#[zbus::proxy(
    interface = "org.freedesktop.login1.Manager",
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1"
)]
pub trait Login1 {
    #[zbus(signal)]
    fn prepare_for_sleep(&self, start: bool) -> zbus::Result<()>;
}

/// Uykudan her uyanışta `on_resume` çağrılır. logind yoksa (systemd'siz dağıtım)
/// sessizce biter; zamanlayıcı yine de 30 sn'de bir saati denetler.
pub async fn watch_resume(on_resume: impl Fn() + Send + 'static) {
    let run = async {
        let conn = zbus::Connection::system().await?;
        let proxy = Login1Proxy::new(&conn).await?;
        let mut s = proxy.receive_prepare_for_sleep().await?;
        while let Some(sig) = crate::dbus::next(&mut s).await {
            if let Ok(a) = sig.args()
                && !*a.start()
            {
                log::info!("resumed from sleep");
                on_resume();
            }
        }
        Ok::<(), zbus::Error>(())
    };
    if let Err(e) = run.await {
        log::info!("logind not available ({e}); relying on periodic clock checks");
    }
}

/// "Rahatsız Etmeyin" açık mı? GNOME: `show-banners = false`; KDE ve diğerleri:
/// bildirim sunucusunun `Inhibited` özelliği.
pub async fn do_not_disturb(conn: &zbus::Connection) -> bool {
    if let Ok(out) = tokio::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.notifications", "show-banners"])
        .output()
        .await
        && out.status.success()
        && String::from_utf8_lossy(&out.stdout).trim() == "false"
    {
        return true;
    }
    if let Ok(p) = crate::notify::NotificationsProxy::new(conn).await
        && let Ok(true) = p.inhibited().await
    {
        return true;
    }
    false
}

/// Sistem saat dilimi (`$TZ` ya da `/etc/localtime` bağlantısı), ör. `Europe/Istanbul`.
pub fn system_timezone() -> Option<String> {
    if let Ok(tz) = std::env::var("TZ")
        && !tz.is_empty()
    {
        return Some(tz.trim_start_matches(':').to_string());
    }
    let link = std::fs::read_link("/etc/localtime").ok()?;
    let s = link.to_string_lossy();
    s.split("zoneinfo/").nth(1).map(|z| z.to_string())
}

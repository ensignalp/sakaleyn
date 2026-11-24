//! Masaüstü bildirimleri (org.freedesktop.Notifications): GNOME, KDE, XFCE,
//! Cinnamon, MATE… hepsinde aynı arayüz. Düğmeler (`actions`) `ActionInvoked`
//! sinyaliyle geri gelir.

use std::collections::HashMap;

use zbus::Variant as Value;

#[zbus::proxy(
    interface = "org.freedesktop.Notifications",
    default_service = "org.freedesktop.Notifications",
    default_path = "/org/freedesktop/Notifications"
)]
pub trait Notifications {
    #[allow(clippy::too_many_arguments)]
    fn notify(
        &self,
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: &[&str],
        hints: HashMap<&str, Value<'_>>,
        expire_timeout: i32,
    ) -> zbus::Result<u32>;
    fn close_notification(&self, id: u32) -> zbus::Result<()>;
    #[zbus(signal)]
    fn action_invoked(&self, id: u32, action_key: String) -> zbus::Result<()>;
    #[zbus(signal)]
    fn notification_closed(&self, id: u32, reason: u32) -> zbus::Result<()>;
    /// KDE: "Rahatsız Etmeyin" açık mı (GNOME'da yok).
    #[zbus(property)]
    fn inhibited(&self) -> zbus::Result<bool>;
}

pub struct Note<'a> {
    pub summary: &'a str,
    pub body: &'a str,
    /// (anahtar, etiket) çiftleri.
    pub actions: &'a [(&'a str, String)],
    pub urgent: bool,
    /// Kullanıcı kapatana kadar kalsın (ezan, kaçırılan vakit değil).
    pub resident: bool,
}

/// Simge: kurulu `sakaleyn` tema simgesi; yoksa dosya yolu.
pub fn icon() -> String {
    let mut dirs = vec![];
    if let Some(d) = std::env::var_os("XDG_DATA_HOME").map(std::path::PathBuf::from).or_else(|| {
        std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/share"))
    }) {
        dirs.push(d);
    }
    for d in std::env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| "/usr/local/share:/usr/share".into())
        .split(':')
    {
        dirs.push(d.into());
    }
    for d in dirs {
        let p = d.join("icons/hicolor/scalable/apps/sakaleyn.svg");
        if p.exists() {
            return "sakaleyn".into();
        }
    }
    String::new()
}

pub async fn send(conn: &zbus::Connection, n: &Note<'_>, replaces: u32) -> zbus::Result<u32> {
    let proxy = NotificationsProxy::new(conn).await?;
    let mut actions: Vec<&str> = Vec::new();
    for (k, label) in n.actions {
        actions.push(k);
        actions.push(label.as_str());
    }
    let mut hints: HashMap<&str, Value<'_>> = HashMap::new();
    hints.insert("desktop-entry", Value::from("sakaleyn"));
    hints.insert("urgency", Value::from(if n.urgent { 2u8 } else { 1u8 }));
    if n.resident {
        hints.insert("resident", Value::from(true));
    }
    // ses uygulamanın kendisinden gelir; bildirim sunucusu ayrıca çalmasın
    hints.insert("suppress-sound", Value::from(true));
    proxy
        .notify("Sakaleyn", replaces, &icon(), n.summary, n.body, &actions, hints, -1)
        .await
}

pub async fn close(conn: &zbus::Connection, id: u32) {
    if let Ok(p) = NotificationsProxy::new(conn).await {
        let _ = p.close_notification(id).await;
    }
}

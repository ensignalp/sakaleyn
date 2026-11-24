//! `sakaleyn tray`: StatusNotifierItem tepsi simgesi (KDE Plasma, XFCE,
//! Cinnamon, MATE, Budgie). GNOME'da gerek yok: eklenti paneli kullanılır.
//! Veri servisten D-Bus ile gelir.

use std::time::Duration;

use chrono::{DateTime, Utc};
use ksni::menu::{CheckmarkItem, StandardItem};
use ksni::{MenuItem, ToolTip, Tray, TrayMethods};
use sakaleyn_core::model::{Lang, Vakit};
use serde_json::Value;

use crate::dbus;
use crate::i18n;

struct SakaleynTray {
    state: Option<Value>,
}

impl SakaleynTray {
    fn lang(&self) -> Lang {
        match self.state.as_ref().and_then(|s| s["lang"].as_str()) {
            Some("en") => Lang::En,
            _ => Lang::Tr,
        }
    }

    fn t(&self, tr: &str, en: &str) -> String {
        i18n::t(self.lang(), tr, en)
    }

    /// "İkindi 15:25 · 1 saat 5 dakika"
    fn next_line(&self) -> Option<String> {
        let s = self.state.as_ref()?;
        let v = Vakit::from_key(s["next"]["vakit"].as_str()?)?;
        let at: DateTime<Utc> = s["next"]["at"].as_str()?.parse().ok()?;
        let mins = ((at - Utc::now()).num_seconds() + 59) / 60;
        Some(format!(
            "{} {} · {}",
            v.name(self.lang()),
            s["next"]["local"].as_str().unwrap_or(""),
            i18n::duration(self.lang(), mins)
        ))
    }
}

fn call(f: impl FnOnce(dbus::SakaleynProxy<'static>) -> std::pin::Pin<Box<dyn Future<Output = ()> + Send>> + Send + 'static) {
    tokio::spawn(async move {
        if let Ok(p) = dbus::proxy().await {
            f(p).await;
        }
    });
}

impl Tray for SakaleynTray {
    const MENU_ON_ACTIVATE: bool = true;

    fn id(&self) -> String {
        "sakaleyn".into()
    }

    fn title(&self) -> String {
        self.next_line().unwrap_or_else(|| "Sakaleyn".into())
    }

    fn icon_name(&self) -> String {
        "sakaleyn".into()
    }

    fn tool_tip(&self) -> ToolTip {
        let description = match &self.state {
            Some(s) => {
                let mut lines = vec![s["district"]["name"].as_str().unwrap_or("").to_string()];
                if let Some(n) = self.next_line() {
                    lines.push(n);
                }
                lines.join("\n")
            }
            None => self.t("Sakaleyn servisi bekleniyor…", "Waiting for the Sakaleyn service…"),
        };
        ToolTip {
            title: "Sakaleyn".into(),
            description,
            icon_name: "sakaleyn".into(),
            icon_pixmap: Vec::new(),
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let mut items: Vec<MenuItem<Self>> = Vec::new();
        let label = |s: String| -> MenuItem<Self> {
            StandardItem {
                label: s.replace('_', "__"),
                enabled: false,
                ..Default::default()
            }
            .into()
        };
        match &self.state {
            Some(s) => {
                let lang = self.lang();
                if let Some(name) = s["district"]["name"].as_str() {
                    items.push(label(format!("{name} · {}", s["today"]["hijri"].as_str().unwrap_or(""))));
                }
                let current = s["current"]["vakit"].as_str().unwrap_or("");
                for v in Vakit::ALL {
                    let time = s["today"]["times"][v.key()].as_str().unwrap_or("--:--");
                    let mark = if v.key() == current { "▸" } else { "  " };
                    items.push(label(format!("{mark} {:<8} {time}", v.name(lang))));
                }
                if let Some(n) = self.next_line() {
                    items.push(MenuItem::Separator);
                    items.push(label(n));
                }
                items.push(MenuItem::Separator);
                if !s["playing"].is_null() {
                    items.push(
                        StandardItem {
                            label: self.t("Sesi durdur", "Stop sound"),
                            icon_name: "media-playback-stop".into(),
                            activate: Box::new(|_: &mut Self| {
                                call(|p| Box::pin(async move { drop(p.stop_audio().await) }))
                            }),
                            ..Default::default()
                        }
                        .into(),
                    );
                }
                let muted = s["muted_today"].as_bool().unwrap_or(false);
                items.push(
                    CheckmarkItem {
                        label: self.t("Bugün sessiz (yalnızca bildirim)", "Silent today (notifications only)"),
                        checked: muted,
                        activate: Box::new(move |_: &mut Self| {
                            call(move |p| Box::pin(async move { drop(p.mute_today(!muted).await) }))
                        }),
                        ..Default::default()
                    }
                    .into(),
                );
            }
            None => items.push(label(self.t("Sakaleyn servisi çalışmıyor", "Sakaleyn service is not running"))),
        }
        items.push(
            StandardItem {
                label: self.t("Ayarlar", "Settings"),
                icon_name: "preferences-system".into(),
                activate: Box::new(|_: &mut Self| {
                    if let Err(e) = crate::open_settings() {
                        log::warn!("{e}");
                    }
                }),
                ..Default::default()
            }
            .into(),
        );
        items.push(MenuItem::Separator);
        items.push(
            StandardItem {
                label: self.t("Simgeyi kapat", "Close icon"),
                icon_name: "application-exit".into(),
                activate: Box::new(|_: &mut Self| std::process::exit(0)),
                ..Default::default()
            }
            .into(),
        );
        items
    }
}

pub async fn run() -> Result<(), String> {
    let handle = SakaleynTray { state: None }.spawn().await.map_err(|e| {
        format!("{e}\n(Tepsi alanı bulunamadı. GNOME'da Sakaleyn eklentisini kullanın.)")
    })?;
    // Geri sayım metni dakikada bir tazelenir.
    let h = handle.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(30)).await;
            h.update(|_| ()).await;
        }
    });
    dbus::watch(move |state| {
        let h = handle.clone();
        tokio::spawn(async move {
            h.update(|t| t.state = Some(state)).await;
        });
    })
    .await
}

//! Ses kataloğu ve oynatıcı.
//!
//! Paketle gelen sesler `…/sakaleyn/sounds/{adhan,alerts}/<id>.ogg` altında,
//! adları `sounds.json`'da. Kullanıcının `~/.local/share/sakaleyn/sounds/`
//! klasörüne koyduğu ses dosyaları `user:<dosya adı>` kimliğiyle listeye girer.
//!
//! Çalma için sistemdeki ilk uygun oynatıcı kullanılır (yerel kütüphane
//! bağımlılığı yok, her dağıtımda çalışır): pw-play (PipeWire), paplay
//! (PulseAudio), gst-play-1.0, ffplay, mpv. `$SAKALEYN_PLAYER` ile seçilebilir.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use sakaleyn_core::model::Lang;
use serde::{Deserialize, Serialize};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

#[derive(Clone, Debug, Serialize)]
pub struct SoundInfo {
    pub id: String,
    /// `adhan`, `alert` veya `user`.
    pub kind: String,
    pub title: String,
    pub path: PathBuf,
}

#[derive(Deserialize)]
struct ManifestEntry {
    id: String,
    kind: String,
    title: String,
    #[serde(default)]
    title_en: String,
}

const AUDIO_EXT: [&str; 7] = ["ogg", "oga", "opus", "mp3", "flac", "wav", "m4a"];

fn kind_dir(kind: &str) -> &'static str {
    if kind == "adhan" { "adhan" } else { "alerts" }
}

/// Bütün sesler: paket sesleri (manifest sırasıyla), sonra kullanıcı sesleri.
pub fn catalog(lang: Lang) -> Vec<SoundInfo> {
    catalog_in(&sakaleyn_core::paths::builtin_sounds_dirs(), &sakaleyn_core::paths::user_sounds_dir(), lang)
}

pub fn catalog_in(builtin: &[PathBuf], user: &Path, lang: Lang) -> Vec<SoundInfo> {
    let mut out: Vec<SoundInfo> = Vec::new();
    for dir in builtin {
        let Ok(bytes) = std::fs::read(dir.join("sounds.json")) else { continue };
        let Ok(entries) = serde_json::from_slice::<Vec<ManifestEntry>>(&bytes) else {
            log::warn!("{}: invalid sounds.json", dir.display());
            continue;
        };
        for e in entries {
            if out.iter().any(|s| s.id == e.id) {
                continue;
            }
            let path = dir.join(kind_dir(&e.kind)).join(format!("{}.ogg", e.id));
            if !path.exists() {
                continue;
            }
            let title = if lang == Lang::En && !e.title_en.is_empty() { e.title_en } else { e.title };
            out.push(SoundInfo {
                id: e.id,
                kind: e.kind,
                title,
                path,
            });
        }
    }
    if let Ok(rd) = std::fs::read_dir(user) {
        let mut files: Vec<_> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.extension()
                    .and_then(|x| x.to_str())
                    .is_some_and(|x| AUDIO_EXT.contains(&x.to_ascii_lowercase().as_str()))
            })
            .collect();
        files.sort();
        for p in files {
            let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
            let title = p.file_stem().unwrap_or_default().to_string_lossy().to_string();
            out.push(SoundInfo {
                id: format!("user:{name}"),
                kind: "user".into(),
                title,
                path: p,
            });
        }
    }
    out
}

/// Ses kimliğinin dosyası. Bilinmeyen kimlik → `None` (bildirim sessiz gelir).
pub fn resolve(id: &str) -> Option<PathBuf> {
    if id.is_empty() || id == "none" {
        return None;
    }
    if let Some(name) = id.strip_prefix("user:") {
        let p = sakaleyn_core::paths::user_sounds_dir().join(name);
        return p.exists().then_some(p);
    }
    catalog(Lang::Tr).into_iter().find(|s| s.id == id).map(|s| s.path)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Backend {
    PwPlay,
    Paplay,
    GstPlay,
    Ffplay,
    Mpv,
}

impl Backend {
    const ALL: [(Backend, &'static str); 5] = [
        (Backend::PwPlay, "pw-play"),
        (Backend::Paplay, "paplay"),
        (Backend::GstPlay, "gst-play-1.0"),
        (Backend::Ffplay, "ffplay"),
        (Backend::Mpv, "mpv"),
    ];

    fn detect() -> Option<(Backend, PathBuf)> {
        let wanted = std::env::var("SAKALEYN_PLAYER").ok().filter(|v| !v.is_empty());
        Self::ALL
            .iter()
            .filter(|(_, name)| wanted.as_deref().is_none_or(|w| w == *name))
            .find_map(|(b, name)| which(name).map(|p| (*b, p)))
    }

    fn args(self, file: &Path, volume: f64) -> Vec<String> {
        let f = file.to_string_lossy().to_string();
        let v = volume.clamp(0.0, 1.0);
        match self {
            // Ses karıştırıcıda "Sakaleyn" adıyla görünsün.
            Backend::PwPlay => vec![
                "-P".into(),
                "{ application.name = \"Sakaleyn\" application.icon-name = \"sakaleyn\" }".into(),
                format!("--volume={v:.2}"),
                f,
            ],
            Backend::Paplay => vec![format!("--volume={}", (v * 65536.0) as u32), f],
            Backend::GstPlay => vec!["--no-interactive".into(), "-q".into(), format!("--volume={v:.2}"), f],
            Backend::Ffplay => vec![
                "-nodisp".into(),
                "-autoexit".into(),
                "-loglevel".into(),
                "quiet".into(),
                "-volume".into(),
                format!("{}", (v * 100.0) as u32),
                f,
            ],
            Backend::Mpv => vec![
                "--no-video".into(),
                "--really-quiet".into(),
                format!("--volume={}", (v * 100.0) as u32),
                f,
            ],
        }
    }
}

fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|d| d.join(name))
        .find(|p| p.is_file())
}

/// Çalan ses hakkında bilgi (durumda gösterilir).
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Playing {
    /// `adhan`, `alert`, `preview`.
    pub kind: String,
    pub vakit: Option<String>,
    pub sound: String,
}

struct Current {
    child: Child,
    info: Playing,
    generation: u64,
}

/// Tek seferde tek ses. Yeni ses öncekini durdurur; ezan çalarken uyarı sesi çalınmaz.
#[derive(Clone)]
pub struct Player {
    inner: Arc<Mutex<Option<Current>>>,
    generation: Arc<std::sync::atomic::AtomicU64>,
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}

impl Player {
    pub fn new() -> Self {
        Player {
            inner: Arc::new(Mutex::new(None)),
            generation: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    pub fn backend_name() -> Option<&'static str> {
        let (b, _) = Backend::detect()?;
        Backend::ALL.iter().find(|(x, _)| *x == b).map(|(_, n)| *n)
    }

    pub async fn playing(&self) -> Option<Playing> {
        let mut g = self.inner.lock().await;
        if let Some(c) = g.as_mut()
            && let Ok(Some(_)) = c.child.try_wait()
        {
            *g = None;
        }
        g.as_ref().map(|c| c.info.clone())
    }

    /// Çalmaya başlar; ses bitince (ya da durdurulunca) `on_end` çağrılır.
    pub async fn play(
        &self,
        file: &Path,
        volume: f64,
        info: Playing,
        on_end: impl FnOnce() + Send + 'static,
    ) -> Result<(), String> {
        let (backend, bin) = Backend::detect().ok_or_else(|| {
            "no audio player found (install pipewire-utils, pulseaudio-utils, gstreamer, ffmpeg or mpv)".to_string()
        })?;
        self.stop().await;
        let child = Command::new(bin)
            .args(backend.args(file, volume))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("{backend:?}: {e}"))?;
        let generation = self.generation.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        log::info!("playing {} ({backend:?}, volume {volume:.2})", file.display());
        *self.inner.lock().await = Some(Current { child, info, generation });

        // Bitişi izle: süreç kendiliğinden biterse ya da durdurulursa on_end.
        let inner = self.inner.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                let mut g = inner.lock().await;
                match g.as_mut() {
                    Some(c) if c.generation == generation => {
                        if let Ok(Some(_)) = c.child.try_wait() {
                            *g = None;
                            break;
                        }
                    }
                    _ => break, // durduruldu ya da yerini başka ses aldı
                }
            }
            on_end();
        });
        Ok(())
    }

    /// Çalan sesi durdurur; bir şey çalıyor idiyse `true`.
    pub async fn stop(&self) -> bool {
        let mut g = self.inner.lock().await;
        if let Some(mut c) = g.take() {
            let _ = c.child.start_kill();
            let _ = c.child.wait().await;
            log::info!("stopped {}", c.info.sound);
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_reads_manifest_and_user_dir() {
        let dir = tempfile::tempdir().unwrap();
        let b = dir.path().join("builtin");
        std::fs::create_dir_all(b.join("adhan")).unwrap();
        std::fs::create_dir_all(b.join("alerts")).unwrap();
        std::fs::write(
            b.join("sounds.json"),
            r#"[{"id":"kabe","kind":"adhan","title":"Kâbe","title_en":"Kaaba"},
                {"id":"nur","kind":"alert","title":"Nur"},
                {"id":"missing","kind":"alert","title":"Yok"}]"#,
        )
        .unwrap();
        std::fs::write(b.join("adhan/kabe.ogg"), b"x").unwrap();
        std::fs::write(b.join("alerts/nur.ogg"), b"x").unwrap();
        let u = dir.path().join("user");
        std::fs::create_dir_all(&u).unwrap();
        std::fs::write(u.join("Benim Ezanım.mp3"), b"x").unwrap();
        std::fs::write(u.join("not.txt"), b"x").unwrap();

        let c = catalog_in(std::slice::from_ref(&b), &u, Lang::En);
        let ids: Vec<_> = c.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["kabe", "nur", "user:Benim Ezanım.mp3"]);
        assert_eq!(c[0].title, "Kaaba");
        assert_eq!(c[2].title, "Benim Ezanım");
        assert_eq!(c[2].kind, "user");
    }

    #[test]
    fn backend_args() {
        let a = Backend::PwPlay.args(Path::new("/x.ogg"), 0.5);
        assert!(a.contains(&"--volume=0.50".to_string()));
        assert_eq!(Backend::Paplay.args(Path::new("/x.ogg"), 1.0)[0], "--volume=65536");
        assert_eq!(Backend::Mpv.args(Path::new("/x.ogg"), 2.0)[2], "--volume=100");
    }
}

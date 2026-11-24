//! Dosya konumları (XDG).

use std::path::PathBuf;

pub fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
}

/// `~/.config/sakaleyn/config.toml`
pub fn config_file() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| home_dir().join(".config"))
        .join("sakaleyn")
        .join("config.toml")
}

/// `~/.cache/sakaleyn`: Diyanet vakit tabloları ve dini günler.
pub fn cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| home_dir().join(".cache"))
        .join("sakaleyn")
}

/// `~/.local/state/sakaleyn`: çalınan olaylar, "bugün sessiz", "kıldım".
pub fn state_dir() -> PathBuf {
    dirs::state_dir()
        .unwrap_or_else(|| home_dir().join(".local/state"))
        .join("sakaleyn")
}

/// `~/.local/share/sakaleyn/sounds`: kullanıcının eklediği sesler.
pub fn user_sounds_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| home_dir().join(".local/share"))
        .join("sakaleyn")
        .join("sounds")
}

/// Paketle gelen ses klasörleri: `$SAKALEYN_SOUNDS_DIR`, sonra `$XDG_DATA_DIRS`
/// (ve `~/.local/share`) altındaki `sakaleyn/sounds`.
pub fn builtin_sounds_dirs() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(p) = std::env::var_os("SAKALEYN_SOUNDS_DIR").filter(|v| !v.is_empty()) {
        out.push(PathBuf::from(p));
    }
    if let Some(d) = dirs::data_dir() {
        out.push(d.join("sakaleyn").join("sounds"));
    }
    let sys = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    for d in sys.split(':').filter(|d| !d.is_empty()) {
        out.push(PathBuf::from(d).join("sakaleyn").join("sounds"));
    }
    out.dedup();
    out
}

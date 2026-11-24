//! Sakaleyn çekirdeği.
//!
//! D-Bus, ses veya masaüstüne bağımlılığı olmayan katman: Diyanet vakitlerini
//! indirip ayrıştırır, önbellekte tutar, ilçe veritabanında arar, Diyanet'e
//! ulaşılamazsa vakitleri astronomik olarak hesaplar ve günün olaylarını
//! (ön uyarı, ezan, …) üretir. Zamanlama ve ses `sakaleyn` binary'sinde yaşar.

pub mod cache;
pub mod calc;
pub mod config;
pub mod districts;
pub mod diyanet;
pub mod http;
pub mod model;
pub mod paths;
pub mod religious;
pub mod schedule;
pub mod text;

/// Sakaleyn sürümü (Cargo.toml'dan).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

//! Ortak HTTP istemcisi.

use std::time::Duration;

pub use reqwest::Client;

pub fn user_agent() -> String {
    format!("sakaleyn/{} (Linux prayer times; +https://github.com/ensignalp/sakaleyn)", crate::VERSION)
}

fn builder() -> reqwest::ClientBuilder {
    Client::builder()
        .user_agent(user_agent())
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
}

/// Sistem sertifika deposunu kullanır; yoksa gömülü Mozilla kök sertifikalarına düşer.
pub fn build_client() -> Client {
    match builder().build() {
        Ok(c) => c,
        Err(e) => {
            log::warn!("system CA certificates unavailable ({e}); using bundled Mozilla roots");
            let roots = webpki_root_certs::TLS_SERVER_ROOT_CERTS
                .iter()
                .filter_map(|c| reqwest::Certificate::from_der(c.as_ref()).ok());
            builder()
                .tls_certs_only(roots)
                .build()
                .unwrap_or_else(|e| panic!("HTTP client could not be built: {e}"))
        }
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum FetchError {
    #[error("network error: {0}")]
    Network(String),
    #[error("HTTP {0}")]
    Http(u16),
    #[error("unexpected page: {0}")]
    Parse(String),
}

/// GET; ağ hatasında (Diyanet sunucuları bağlantıyı ara sıra kesiyor) iki kez daha dener.
pub async fn get_text(client: &Client, url: &str) -> Result<String, FetchError> {
    let mut last = FetchError::Network("not attempted".into());
    for attempt in 0..3u64 {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_secs(2 * attempt)).await;
        }
        match get_once(client, url).await {
            Err(e @ FetchError::Network(_)) => last = e,
            other => return other,
        }
    }
    Err(last)
}

async fn get_once(client: &Client, url: &str) -> Result<String, FetchError> {
    let resp = client.get(url).send().await.map_err(|e| FetchError::Network(e.to_string()))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(FetchError::Http(status.as_u16()));
    }
    resp.text().await.map_err(|e| FetchError::Network(e.to_string()))
}

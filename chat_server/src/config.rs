use anyhow::Result;
use chat_core::load_config;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub auth: AuthConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerConfig {
    pub port: u16,
    pub db_url: String,
    pub base_dir: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthConfig {
    pub sk: String,
    pub pk: String,
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        load_config("CHAT_CONFIG", "app.yaml")
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn load_config() {
        let cfg = AppConfig::load().unwrap();

        assert_eq!(6666, cfg.server.port);
    }
}

use anyhow::Result;
use chat_core::load_config;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::path::PathBuf;
use tracing::Level;

#[derive(Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub auth: AuthConfig,
    #[serde(
        serialize_with = "from_level",
        deserialize_with = "into_level",
        default = "debug_level"
    )]
    pub log_level: Level,
}

fn debug_level() -> Level {
    Level::DEBUG
}

fn from_level<S>(v: &Level, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(v.as_str())
}

fn into_level<'de, D>(deserializer: D) -> Result<Level, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;
    String::deserialize(deserializer)
        .and_then(|s| s.parse::<Level>().map_err(|e| Error::custom(e.to_string())))
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

        assert_eq!(5555, cfg.server.port);
        assert_eq!(Level::INFO, cfg.log_level);
    }
}

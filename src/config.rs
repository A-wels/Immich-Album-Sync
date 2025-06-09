use std::path::PathBuf;
use std::fs;
use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub api_url: String,
    pub api_key: String,
    pub album_id: String,
    pub local_folder: String,
    pub interval_minutes: Option<u64>,
    pub background_interval_minutes: Option<u64>,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    Io(#[from] std::io::Error),
    #[error("Failed to parse config JSON: {0}")]
    Json(#[from] serde_json::Error),
}

impl Config {
    pub fn load(path: &PathBuf) -> Result<Self, ConfigError> {
        let data = fs::read_to_string(path)?;
        let mut config: Config = serde_json::from_str(&data)?;
        // Ensure /api at end
        if !config.api_url.trim_end_matches('/').ends_with("/api") {
            config.api_url = format!("{}/api", config.api_url.trim_end_matches('/'));
        }
        Ok(config)
    }
}

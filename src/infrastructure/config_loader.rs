use crate::domain::entities::Config;
use crate::domain::ports::ConfigPort;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;

#[derive(Deserialize)]
struct TomlConfig {
    #[serde(default = "default_api_port")]
    api_port: u16,
    #[serde(default = "default_secret_token")]
    secret_token: String,
    #[serde(default = "default_disk_threshold")]
    disk_threshold: u8,
    #[serde(default)]
    docker_services: Option<Vec<String>>,
    #[serde(default)]
    webhook_url: Option<String>,
}

fn default_api_port() -> u16 {
    3000
}
fn default_secret_token() -> String {
    "change-me".to_string()
}
fn default_disk_threshold() -> u8 {
    90
}

pub struct FileConfigLoader;

impl ConfigPort for FileConfigLoader {
    fn load_config(&self, path: &str) -> Result<Config> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file at {}", path))?;

        let toml_config: TomlConfig =
            toml::from_str(&content).context("Failed to parse config file")?;

        Ok(Config {
            api_port: toml_config.api_port,
            secret_token: toml_config.secret_token,
            disk_threshold: toml_config.disk_threshold,
            docker_services: toml_config.docker_services,
            webhook_url: toml_config.webhook_url,
        })
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            api_port: default_api_port(),
            secret_token: default_secret_token(),
            disk_threshold: default_disk_threshold(),
            docker_services: None,
            webhook_url: None,
        }
    }
}

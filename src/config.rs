use serde::Deserialize;
use std::fs;
use anyhow::{Context, Result};

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub api_port: u16,
    pub secret_token: String,
    pub disk_threshold: u8,
    pub docker_services: Vec<String>,
}

impl Config {
    pub fn load(path: &str) -> Result<Self> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file at {}", path))?;
        
        let config: Config = toml::from_str(&content)
            .context("Failed to parse config file")?;
            
        Ok(config)
    }
}

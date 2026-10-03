use crate::domain::entities::Config;
use crate::domain::ports::ConfigPort;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Deserialize, Serialize, Clone)]
pub struct TomlWebhook {
    url: String,
    provider: String,
}

#[derive(Deserialize, Serialize)]
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
    webhooks: Option<Vec<TomlWebhook>>,
}

fn default_api_port() -> u16 {
    3000
}
/// No usable default: the `/webhook` endpoint stays disabled until a real token is configured.
fn default_secret_token() -> String {
    String::new()
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

        let webhooks = toml_config
            .webhooks
            .unwrap_or_default()
            .into_iter()
            .map(|w| {
                let provider = match w.provider.to_lowercase().as_str() {
                    "discord" => crate::domain::entities::Provider::Discord,
                    "slack" => crate::domain::entities::Provider::Slack,
                    "teams" => crate::domain::entities::Provider::Teams,
                    _ => crate::domain::entities::Provider::Discord, // Default or error? Defaulting to Discord for now safely
                };
                crate::domain::entities::WebhookConfig {
                    url: w.url,
                    provider,
                }
            })
            .collect();

        Ok(Config {
            api_port: toml_config.api_port,
            secret_token: toml_config.secret_token,
            disk_threshold: toml_config.disk_threshold,
            docker_services: toml_config.docker_services,
            webhooks,
        })
    }
}

impl FileConfigLoader {
    pub fn add_webhook(&self, path: &str, url: String, provider: String) -> Result<()> {
        let content = fs::read_to_string(path).unwrap_or_else(|_| "".to_string());

        // If file doesn't exist or is empty, start fresh? Or assume it exists from load?
        // Let's try to parse, if fail, assume default
        let mut toml_config: TomlConfig = toml::from_str(&content).unwrap_or(TomlConfig {
            api_port: default_api_port(),
            secret_token: default_secret_token(),
            disk_threshold: default_disk_threshold(),
            docker_services: None,
            webhooks: None,
        });

        let new_webhook = TomlWebhook { url, provider };

        match &mut toml_config.webhooks {
            Some(hooks) => hooks.push(new_webhook),
            None => toml_config.webhooks = Some(vec![new_webhook]),
        }

        let new_content = toml::to_string(&toml_config).context("Failed to serialize config")?;
        fs::write(path, new_content).context("Failed to write config file")?;
        Ok(())
    }

    pub fn list_webhooks(&self, path: &str) -> Result<Vec<(String, String)>> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file at {}", path))?;
        let toml_config: TomlConfig =
            toml::from_str(&content).context("Failed to parse config file")?;

        Ok(toml_config
            .webhooks
            .unwrap_or_default()
            .into_iter()
            .map(|w| (w.url, w.provider))
            .collect())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            api_port: default_api_port(),
            secret_token: default_secret_token(),
            disk_threshold: default_disk_threshold(),
            docker_services: None,
            webhooks: vec![],
        }
    }
}

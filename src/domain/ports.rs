use crate::domain::entities::{Action, Alert, Config, HealthStatus};
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait MonitorPort: Send + Sync {
    fn check_disk_usage(&self) -> u8;
    fn check_service(&self, name: &str) -> HealthStatus;
    fn discover_services(&self) -> Vec<String>;
}

#[async_trait]
pub trait RemediationPort: Send + Sync {
    async fn heal(&self, action: Action) -> Result<()>;
}

#[async_trait]
pub trait NotificationPort: Send + Sync {
    async fn send_alert(&self, webhook_url: &str, alert: Alert);
}

pub trait ConfigPort: Send + Sync {
    fn load_config(&self, path: &str) -> Result<Config>;
}

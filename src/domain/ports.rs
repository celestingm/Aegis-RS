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
#[async_trait]
pub trait NotificationPort: Send + Sync {
    async fn send_alert(&self, config: &crate::domain::entities::WebhookConfig, alert: Alert);
    async fn send_status_report(&self, config: &crate::domain::entities::WebhookConfig, message: String, image_data: Option<Vec<u8>>, previous_message_id: Option<&str>) -> Result<Option<String>>;
}

pub trait ConfigPort: Send + Sync {
    fn load_config(&self, path: &str) -> Result<Config>;
}

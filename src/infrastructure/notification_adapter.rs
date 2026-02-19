use crate::domain::entities::Alert;
use crate::domain::ports::NotificationPort;
use async_trait::async_trait;
use reqwest::Client;
use tracing::{error, info};

pub struct WebhookNotifier;

#[async_trait]
impl NotificationPort for WebhookNotifier {
    async fn send_alert(&self, webhook_url: &str, alert: Alert) {
        let client = Client::new();
        let result = client.post(webhook_url).json(&alert).send().await;

        match result {
            Ok(res) => {
                if res.status().is_success() {
                    info!("Alert sent successfully to webhook.");
                } else {
                    error!(
                        "Failed to send alert: Webhook returned status {}",
                        res.status()
                    );
                }
            }
            Err(e) => {
                error!("Failed to send alert: {}", e);
            }
        }
    }
}

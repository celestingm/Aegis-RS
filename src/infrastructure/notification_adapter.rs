use crate::domain::entities::{Alert, Config};
use crate::domain::ports::NotificationPort;
use async_trait::async_trait;
use reqwest::Client;
use tracing::{error, info};

pub struct WebhookNotifier {
    _config: Config,
}

impl WebhookNotifier {
    pub fn new(config: Config) -> Self {
        Self { _config: config }
    }
}

#[async_trait]
impl NotificationPort for WebhookNotifier {
    async fn send_alert(&self, config: &crate::domain::entities::WebhookConfig, alert: Alert) {
        let client = Client::new();
        // Alerts are generally just POST JSON for all providers for now
        // But we could add provider-specific formatting later
        let result = client.post(&config.url).json(&alert).send().await;
        self.handle_result(result);
    }
    async fn send_status_report(&self, config: &crate::domain::entities::WebhookConfig, message: String, image_data: Option<Vec<u8>>, previous_message_id: Option<&str>) -> anyhow::Result<Option<String>> {
        let client = Client::new();
        
        match config.provider {
            crate::domain::entities::Provider::Discord => {
                self.send_discord_status(&client, &config.url, message, image_data, previous_message_id).await
            },
            crate::domain::entities::Provider::Slack => {
                let payload = serde_json::json!({ "text": message });
                let result = client.post(&config.url).json(&payload).send().await;
                self.handle_provider_result(config.provider.clone(), result);
                Ok(None)
            },
            crate::domain::entities::Provider::Teams => {
                // Teams Workflows often accept Adaptive Cards better than plain text
                let payload = serde_json::json!({
                    "type": "message",
                    "attachments": [
                        {
                            "contentType": "application/vnd.microsoft.card.adaptive",
                            "contentUrl": null,
                            "content": {
                                "$schema": "http://adaptivecards.io/schemas/adaptive-card.json",
                                "type": "AdaptiveCard",
                                "version": "1.4",
                                "body": [
                                    {
                                        "type": "TextBlock",
                                        "text": message,
                                        "wrap": true
                                    }
                                ]
                            }
                        }
                    ]
                });
                let result = client.post(&config.url).json(&payload).send().await;
                self.handle_provider_result(config.provider.clone(), result);
                Ok(None)
            }
        }
    }
}

impl WebhookNotifier {
    async fn send_discord_status(&self, client: &Client, webhook_url: &str, message: String, image_data: Option<Vec<u8>>, previous_message_id: Option<&str>) -> anyhow::Result<Option<String>> {
         if let Some(msg_id) = previous_message_id {
            // EDIT existing message
            let edit_url = format!("{}/messages/{}", webhook_url, msg_id);

            if let Some(image_bytes) = image_data {
                 // Multipart edit to replace attachment
                let part = reqwest::multipart::Part::bytes(image_bytes)
                    .file_name("status.png")
                    .mime_str("image/png")
                    .unwrap();

                 let payload = serde_json::json!({
                    "content": message,
                    "embeds": [{
                        "image": { "url": "attachment://status.png" },
                        "color": 5763719
                    }],
                    "attachments": [{ "id": 0, "filename": "status.png" }]
                });

                let form = reqwest::multipart::Form::new()
                    .text("payload_json", payload.to_string())
                    .part("files[0]", part);

                let result = client.patch(&edit_url).multipart(form).send().await;
                if let Err(e) = result {
                    error!("Failed to edit notification: {}", e);
                    return Ok(None);
                }
            } else {
                 let payload = serde_json::json!({ "content": message });
                 let _ = client.patch(&edit_url).json(&payload).send().await;
            }
            Ok(Some(msg_id.to_string()))
        } else {
            // CREATE new message
            let create_url = format!("{}?wait=true", webhook_url);

            let response = if let Some(image_bytes) = image_data {
                let part = reqwest::multipart::Part::bytes(image_bytes)
                    .file_name("status.png")
                    .mime_str("image/png")
                    .unwrap();

                let payload = serde_json::json!({
                    "content": message,
                    "embeds": [{
                        "image": { "url": "attachment://status.png" },
                        "color": 5763719
                    }],
                    "attachments": [{ "id": 0, "filename": "status.png" }]
                });

                let form = reqwest::multipart::Form::new()
                    .text("payload_json", payload.to_string())
                    .part("files[0]", part);

                client.post(&create_url).multipart(form).send().await
            } else {
                let payload = serde_json::json!({ "content": message });
                client.post(&create_url).json(&payload).send().await
            };

            match response {
                Ok(res) => {
                    if res.status().is_success() {
                         if let Ok(json) = res.json::<serde_json::Value>().await {
                            if let Some(id) = json.get("id").and_then(|id| id.as_str()) {
                                info!("Notification sent successfully, ID: {}", id);
                                return Ok(Some(id.to_string()));
                            }
                         }
                    } else {
                        error!("Failed to send notification: Status {}", res.status());
                    }
                }
                Err(e) => error!("Failed to send notification: {}", e),
            }
            Ok(None)
        }
    }

    fn handle_result(&self, result: Result<reqwest::Response, reqwest::Error>) {
        match result {
            Ok(res) => {
                if res.status().is_success() {
                    info!("Notification sent successfully.");
                } else {
                    error!("Failed to send notification: Status {}", res.status());
                }
            }
            Err(e) => {
                error!("Failed to send notification: {}", e);
            }
        }
    }

    fn handle_provider_result(&self, provider: crate::domain::entities::Provider, result: Result<reqwest::Response, reqwest::Error>) {
         match result {
            Ok(res) => {
                if res.status().is_success() {
                    info!("Notification sent to {:?} successfully.", provider);
                } else {
                    // We can't await body here easily without moving ownership or async, 
                    // but we can log status. To log body we'd need async context passed down or handle inline.
                    // For now, inline in match arms above was better for body.
                    // Actually, let's keep it simple here.
                    error!("Failed to send to {:?}: Status {}", provider, res.status());
                }
            }
            Err(e) => error!("Failed to send to {:?}: {}", provider, e),
        }
    }
}

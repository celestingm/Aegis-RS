use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum HealthStatus {
    Healthy,
    Critical(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct ServiceStatus {
    pub name: String,
    pub status: HealthStatus,
}

#[derive(Debug, Clone, Serialize)]
pub struct SystemMetrics {
    pub disk_usage_percent: u8,
    pub services: Vec<ServiceStatus>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum Provider {
    Discord,
    Slack,
    Teams,
}

#[derive(Debug, Clone, Serialize)]
pub struct WebhookConfig {
    pub url: String,
    pub provider: Provider,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub api_port: u16,
    pub secret_token: String,
    pub disk_threshold: u8,
    pub docker_services: Option<Vec<String>>,
    pub webhooks: Vec<WebhookConfig>,
}

#[derive(Debug, Clone)]
pub enum Action {
    RestartDockerService(String),
    CleanLogs,
}

#[derive(Debug, Clone, Serialize)]
pub struct Alert {
    pub text: String,
    pub level: String,
    pub timestamp: String,
    pub data: Option<AlertData>,
    pub graph_link: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AlertData {
    pub service: String,
    pub action: String,
    pub details: Option<String>,
}

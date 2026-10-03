use crate::domain::entities::{Action, Config, HealthStatus, SystemMetrics};
use crate::domain::ports::RemediationPort;
use crate::infrastructure::remediation_adapter::SystemRemediator;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Html,
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tower_http::trace::TraceLayer;
use tracing::{info, warn};

#[derive(Clone)]
pub struct AppState {
    pub metrics: Arc<Mutex<SystemMetrics>>,
    pub history: Arc<Mutex<VecDeque<SystemMetrics>>>,
    pub secret_token: String,
    pub api_port: u16,
    pub config: Arc<Mutex<Config>>,
}

pub async fn start_server(state: AppState) {
    let port = state.api_port;
    let app = Router::new()
        .route("/metrics", get(get_metrics))
        .route("/graph", get(get_graph))
        .route("/webhook", post(handle_webhook))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    info!("API Server listening on {}", addr);
    axum::serve(listener, app).await.unwrap();
}

use crate::application::graph_generator;

async fn get_graph(State(state): State<AppState>) -> Html<String> {
    let history = state.history.lock().unwrap();
    let metrics = state.metrics.lock().unwrap();

    let svg_content = graph_generator::generate_svg_string(&history, &metrics)
        .unwrap_or_else(|e| format!("<svg><text>Error generating graph: {}</text></svg>", e));

    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>Aegis-RS System Status</title>
    <meta http-equiv="refresh" content="5">
    <style>
        body {{ background-color: #1e1e2e; display: flex; justify-content: center; align-items: center; height: 100vh; margin: 0; }}
    </style>
</head>
<body>
    {}
</body>
</html>"#,
        svg_content
    );

    Html(html)
}

async fn get_metrics(State(state): State<AppState>) -> String {
    let metrics = state.metrics.lock().unwrap();
    let mut output = String::new();

    output.push_str("# HELP disk_usage_percent Disk usage percentage\n");
    output.push_str("# TYPE disk_usage_percent gauge\n");
    output.push_str(&format!(
        "disk_usage_percent {}\n",
        metrics.disk_usage_percent
    ));

    output.push_str("# HELP service_status Status of docker services (0=down, 1=up)\n");
    output.push_str("# TYPE service_status gauge\n");

    for service in &metrics.services {
        let val = match service.status {
            HealthStatus::Healthy => 1,
            _ => 0,
        };
        output.push_str(&format!(
            "service_status{{name=\"{}\"}} {}\n",
            service.name, val
        ));
    }

    output
}

/// Compares two byte slices without short-circuiting on the first difference.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Docker container names: alphanumerics plus `_ . -`, never starting with `-` (option injection).
fn is_valid_service_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && !name.starts_with('-')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
}

async fn run_remediation(remediator: SystemRemediator, action: Action) {
    if let Err(e) = remediator.heal(action).await {
        warn!("Remediation failed: {}", e);
    }
}

async fn handle_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> (StatusCode, Json<Value>) {
    if !state.config.lock().unwrap().has_secure_token() {
        warn!("Webhook request rejected: no secure secret_token configured");
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error": "webhook_disabled"})),
        );
    }

    let auth_header = headers.get("Authorization").and_then(|h| h.to_str().ok());

    match auth_header {
        Some(header_val)
            if constant_time_eq(
                header_val.as_bytes(),
                format!("Bearer {}", state.secret_token).as_bytes(),
            ) => {}
        _ => {
            warn!("Unauthorized webhook attempt");
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "unauthorized"})),
            );
        }
    }

    info!("Received secured webhook: {:?}", payload);

    let config = state.config.lock().unwrap().clone();
    let remediator = SystemRemediator::new(config);

    if let Some(action_str) = payload.get("action").and_then(|v| v.as_str()) {
        match action_str {
            "restart" => {
                if let Some(target) = payload.get("target").and_then(|v| v.as_str()) {
                    if !is_valid_service_name(target) {
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(json!({"error": "invalid_target"})),
                        );
                    }
                    let action = Action::RestartDockerService(target.to_string());
                    tokio::spawn(run_remediation(remediator, action));
                    return (StatusCode::OK, Json(json!({"status": "action_triggered"})));
                }
            }
            "clean_logs" => {
                let action = Action::CleanLogs;
                tokio::spawn(run_remediation(remediator, action));
                return (StatusCode::OK, Json(json!({"status": "action_triggered"})));
            }
            _ => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "unknown_action"})),
                );
            }
        }
    }

    (
        StatusCode::BAD_REQUEST,
        Json(json!({"error": "invalid_payload"})),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_time_eq_matches_equality() {
        assert!(constant_time_eq(b"Bearer abc", b"Bearer abc"));
        assert!(!constant_time_eq(b"Bearer abc", b"Bearer abd"));
        assert!(!constant_time_eq(b"Bearer abc", b"Bearer ab"));
    }

    #[test]
    fn service_name_validation() {
        assert!(is_valid_service_name("grafana"));
        assert!(is_valid_service_name("my_app-1.web"));
        assert!(!is_valid_service_name(""));
        assert!(!is_valid_service_name("--all"));
        assert!(!is_valid_service_name("a b"));
        assert!(!is_valid_service_name("a;rm"));
    }

    #[test]
    fn placeholder_tokens_are_rejected() {
        let mut c = Config::default();
        assert!(!c.has_secure_token());
        c.secret_token = "change-me".into();
        assert!(!c.has_secure_token());
        c.secret_token = "change-me-to-a-secure-token".into();
        assert!(!c.has_secure_token());
        c.secret_token = "a-real-long-random-token".into();
        assert!(c.has_secure_token());
    }
}

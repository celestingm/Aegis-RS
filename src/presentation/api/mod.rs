use crate::domain::entities::{Action, HealthStatus, SystemMetrics};
use crate::domain::ports::RemediationPort;
use crate::infrastructure::remediation_adapter::SystemRemediator;
use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
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
    pub history: Arc<Mutex<VecDeque<u8>>>,
    pub secret_token: String,
    pub api_port: u16,
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

async fn get_graph(State(state): State<AppState>) -> impl IntoResponse {
    let history = state.history.lock().unwrap();
    let metrics = state.metrics.lock().unwrap();

    let svg_content = graph_generator::generate_svg_string(&history, &metrics)
        .unwrap_or_else(|e| format!("<svg><text>Error generating graph: {}</text></svg>", e));

    ([(header::CONTENT_TYPE, "image/svg+xml")], svg_content)
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

async fn handle_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let auth_header = headers.get("Authorization").and_then(|h| h.to_str().ok());

    match auth_header {
        Some(header_val) if header_val == format!("Bearer {}", state.secret_token) => {}
        _ => {
            warn!("Unauthorized webhook attempt");
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "unauthorized"})),
            );
        }
    }

    info!("Received secured webhook: {:?}", payload);

    let remediator = SystemRemediator;

    if let Some(action_str) = payload.get("action").and_then(|v| v.as_str()) {
        match action_str {
            "restart" => {
                if let Some(target) = payload.get("target").and_then(|v| v.as_str()) {
                    let action = Action::RestartDockerService(target.to_string());
                    tokio::spawn(async move {
                        remediator.heal(action).await.unwrap();
                    });
                    return (StatusCode::OK, Json(json!({"status": "action_triggered"})));
                }
            }
            "clean_logs" => {
                let action = Action::CleanLogs;
                tokio::spawn(async move {
                    remediator.heal(action).await.unwrap();
                });
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

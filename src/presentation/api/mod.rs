use std::sync::{Arc, Mutex};
use std::collections::VecDeque;
use axum::{
    routing::{get, post},
    Router, Json, http::{StatusCode, HeaderMap, header}, extract::State,
    response::IntoResponse,
};
use tower_http::trace::TraceLayer;
use serde_json::{json, Value};
use tracing::{info, warn};
use crate::domain::entities::{SystemMetrics, HealthStatus, Action};
use crate::infrastructure::remediation_adapter::SystemRemediator;
use crate::domain::ports::RemediationPort;

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

async fn get_graph(State(state): State<AppState>) -> impl IntoResponse {
    let history = state.history.lock().unwrap();
    let width = 600;
    let height = 200;
    
    if history.is_empty() {
        return ([(header::CONTENT_TYPE, "image/svg+xml")], "<svg></svg>".to_string());
    }

    let points: Vec<(usize, usize)> = history.iter()
        .enumerate()
        .map(|(i, &val)| {
            let x = i * (width as usize / 60); 
            let val_clamped = if val > 100 { 100 } else { val };
            let y = height as usize - ((val_clamped as usize) * height as usize / 100);
            (x, y)
        })
        .collect();

    let mut svg = String::new();
    svg.push_str(&format!("<svg width=\"{}\" height=\"{}\" xmlns=\"http://www.w3.org/2000/svg\">", width, height));
    svg.push_str("<rect width=\"100%\" height=\"100%\" fill=\"#f0f0f0\"/>");
    
    for i in 1..5 {
        let y = i * (height / 5);
        svg.push_str(&format!("<line x1=\"0\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#ccc\" stroke-dasharray=\"2\"/>", y, width, y));
    }

    svg.push_str("<polyline fill=\"none\" stroke=\"#007bff\" stroke-width=\"2\" points=\"");
    for (x, y) in points {
        svg.push_str(&format!("{},{} ", x, y));
    }
    svg.push_str("\"/></svg>");

    ([(header::CONTENT_TYPE, "image/svg+xml")], svg)
}

async fn get_metrics(State(state): State<AppState>) -> String {
    let metrics = state.metrics.lock().unwrap();
    let mut output = String::new();
    
    output.push_str("# HELP disk_usage_percent Disk usage percentage\n");
    output.push_str("# TYPE disk_usage_percent gauge\n");
    output.push_str(&format!("disk_usage_percent {}\n", metrics.disk_usage_percent));

    output.push_str("# HELP service_status Status of docker services (0=down, 1=up)\n");
    output.push_str("# TYPE service_status gauge\n");

    for service in &metrics.services {
        let val = match service.status {
            HealthStatus::Healthy => 1,
            _ => 0,
        };
        output.push_str(&format!("service_status{{name=\"{}\"}} {}\n", service.name, val));
    }

    output
}

async fn handle_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>
) -> (StatusCode, Json<Value>) {
    let auth_header = headers.get("Authorization")
        .and_then(|h| h.to_str().ok());

    match auth_header {
        Some(header_val) if header_val == format!("Bearer {}", state.secret_token) => {
        }
        _ => {
            warn!("Unauthorized webhook attempt");
            return (StatusCode::UNAUTHORIZED, Json(json!({"error": "unauthorized"})));
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
                return (StatusCode::BAD_REQUEST, Json(json!({"error": "unknown_action"})));
            }
        }
    }

    (StatusCode::BAD_REQUEST, Json(json!({"error": "invalid_payload"})))
}

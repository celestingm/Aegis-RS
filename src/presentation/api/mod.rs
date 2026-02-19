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

async fn get_graph(State(state): State<AppState>) -> impl IntoResponse {
    let history = state.history.lock().unwrap();
    let metrics = state.metrics.lock().unwrap();

    let width = 800;
    let height = 400;
    let graph_height = 150;
    let graph_width = 700;
    let graph_x = 50;
    let graph_y = 100;

    let points: Vec<(usize, usize)> = history
        .iter()
        .enumerate()
        .map(|(i, &val)| {
            let x = graph_x + (i as f64 * (graph_width as f64 / 59.0)).round() as usize;
            let val_clamped = if val > 100 { 100 } else { val };
            let y = (graph_y + graph_height) - ((val_clamped as usize) * graph_height / 100);
            (x, y)
        })
        .collect();

    let mut svg = String::new();
    svg.push_str(&format!(
        "<svg width=\"{}\" height=\"{}\" xmlns=\"http://www.w3.org/2000/svg\" style=\"font-family: sans-serif;\">",
        width, height
    ));

    svg.push_str("<rect width=\"100%\" height=\"100%\" fill=\"#1e1e2e\"/>");

    svg.push_str(&format!(
        "<text x=\"{}\" y=\"50\" fill=\"#cdd6f4\" font-size=\"24\" font-weight=\"bold\">Aegis-RS Dashboard</text>",
        graph_x
    ));

    let all_healthy = metrics
        .services
        .iter()
        .all(|s| matches!(s.status, HealthStatus::Healthy))
        && metrics.disk_usage_percent < 90;
    let (status_color, status_text) = if all_healthy {
        ("#a6e3a1", "SYSTEM HEALTHY")
    } else {
        ("#f38ba8", "SYSTEM CRITICAL")
    };

    svg.push_str(&format!(
        "<rect x=\"{}\" y=\"25\" width=\"200\" height=\"40\" rx=\"5\" fill=\"{}\"/>",
        width - 250,
        status_color
    ));
    svg.push_str(&format!(
        "<text x=\"{}\" y=\"52\" fill=\"#1e1e2e\" font-size=\"16\" font-weight=\"bold\" text-anchor=\"middle\">{}</text>",
        width - 150, status_text
    ));

    svg.push_str(
        "<text x=\"50\" y=\"90\" fill=\"#bac2de\" font-size=\"16\">Disk Usage (Last 60m)</text>",
    );

    svg.push_str(&format!(
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#313244\" rx=\"5\"/>",
        graph_x, graph_y, graph_width, graph_height
    ));

    for i in 0..5 {
        let y = graph_y + i * (graph_height / 4);
        svg.push_str(&format!(
            "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#45475a\" stroke-dasharray=\"4\"/>",
            graph_x, y, graph_x + graph_width, y
        ));
    }

    svg.push_str("<polyline fill=\"none\" stroke=\"#89b4fa\" stroke-width=\"2\" points=\"");
    for (x, y) in points {
        svg.push_str(&format!("{},{} ", x, y));
    }
    svg.push_str("\"/>");

    svg.push_str(&format!(
        "<text x=\"{}\" y=\"280\" fill=\"#cdd6f4\" font-size=\"14\">Current Usage: {}%</text>",
        graph_x, metrics.disk_usage_percent
    ));

    let start_y = 320;
    svg.push_str(&format!(
        "<text x=\"{}\" y=\"{}\" fill=\"#bac2de\" font-size=\"16\">Monitored Services</text>",
        graph_x, start_y
    ));

    let mut service_x = graph_x;
    for service in &metrics.services {
        let (color, _status_char) = match service.status {
            HealthStatus::Healthy => ("#a6e3a1", "✓"),
            _ => ("#f38ba8", "✗"),
        };

        svg.push_str(&format!(
            "<rect x=\"{}\" y=\"{}\" width=\"160\" height=\"30\" rx=\"5\" fill=\"#313244\" stroke=\"{}\" stroke-width=\"1\"/>",
            service_x, start_y + 15, color
        ));

        svg.push_str(&format!(
            "<circle cx=\"{}\" cy=\"{}\" r=\"5\" fill=\"{}\"/>",
            service_x + 15,
            start_y + 30,
            color
        ));

        svg.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" fill=\"#cdd6f4\" font-size=\"12\">{}</text>",
            service_x + 30,
            start_y + 35,
            service.name
        ));

        service_x += 180;
    }

    if metrics.services.is_empty() {
        svg.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" fill=\"#6c7086\" font-size=\"14\" font-style=\"italic\">No services monitored (add 'aegis.monitor=true' label)</text>",
            graph_x, start_y + 35
        ));
    }

    svg.push_str("</svg>");

    ([(header::CONTENT_TYPE, "image/svg+xml")], svg)
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

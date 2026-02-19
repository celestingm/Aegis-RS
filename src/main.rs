mod domain;
mod infrastructure;
mod application;
mod presentation;

use std::sync::{Arc, Mutex};
use std::collections::VecDeque; 
use clap::Parser;
use tracing::{info, error, Level};
use tracing_subscriber::FmtSubscriber;
use crate::domain::entities::{SystemMetrics, Config};
use crate::domain::ports::ConfigPort;
use crate::infrastructure::config_loader::FileConfigLoader;
use crate::infrastructure::docker_monitor::DockerMonitor;
use crate::infrastructure::disk_monitor::DiskMonitor;
use crate::infrastructure::remediation_adapter::SystemRemediator;
use crate::infrastructure::notification_adapter::WebhookNotifier;
use crate::application::orchestrator::Orchestrator;
use crate::presentation::api::{start_server, AppState};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value = "config.toml")]
    config: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("setting default subscriber failed");

    let args = Args::parse();
    info!("Starting Aegis-RS (Clean Architecture Edition)...");

    let config_loader = FileConfigLoader;
    let config = config_loader.load_config(&args.config).unwrap_or_else(|e| {
        error!("Failed to load config: {}. Using defaults.", e);
        Config::default()
    });
    info!("Configuration loaded.");

    let metrics = Arc::new(Mutex::new(SystemMetrics {
        disk_usage_percent: 0,
        services: vec![],
    }));

    let history = Arc::new(Mutex::new(VecDeque::with_capacity(60)));
    {
        let mut h = history.lock().unwrap();
        for _ in 0..60 { h.push_back(0); }
    }

    let docker_monitor = Arc::new(DockerMonitor::new());
    let disk_monitor = Arc::new(DiskMonitor::new());
    let remediator = Arc::new(SystemRemediator);
    let notifier = Arc::new(WebhookNotifier);

    let orchestrator = Orchestrator::new(
        config.clone(),
        docker_monitor,
        disk_monitor,
        remediator,
        notifier,
        metrics.clone(),
        history.clone(),
    );

    let app_state = AppState {
        metrics: metrics.clone(),
        history: history.clone(),
        secret_token: config.secret_token.clone(),
        api_port: config.api_port,
    };

    let server_handle = tokio::spawn(async move {
        start_server(app_state).await;
    });

    let orchestrator_handle = tokio::spawn(async move {
        orchestrator.run().await;
    });

    tokio::select! {
        _ = server_handle => error!("API server task matched"),
        _ = orchestrator_handle => error!("Orchestrator task matched"),
    }

    Ok(())
}

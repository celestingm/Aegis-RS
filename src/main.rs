mod monitor;
mod remediation;
mod api;
mod config;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::collections::HashMap;
use tokio::time;
use tracing::{info, warn, error, Level};
use tracing_subscriber::FmtSubscriber;
use crate::monitor::{SystemMetrics, ServiceStatus, HealthStatus, check_disk_usage, check_docker_service};
use crate::remediation::{Action, heal};
use crate::api::{AppState, start_server};
use crate::config::Config;

/// Main entry point for Aegis-RS.
///
/// Initializes the logging system, shared state, and spawns two main async tasks:
/// 1. **API Server**: Handles external requests (metrics, webhooks).
/// 2. **Internal Watcher**: Periodically checks system health and triggers remediation.
///
/// Uses `tokio::select!` to keep the main process alive as long as the tasks are running.
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("setting default subscriber failed");

    info!("Starting Aegis-RS...");

    // Load Configuration
    let config = Config::load("config.toml").unwrap_or_else(|e| {
        error!("Failed to load config.toml: {}. Using separate tool to generate default config if missing.", e);
        panic!("Config load failed");
    });
    info!("Configuration loaded. Monitoring services: {:?}", config.docker_services);

    // Shared state
    let metrics = Arc::new(Mutex::new(SystemMetrics {
        disk_usage_percent: 0,
        services: vec![],
    }));

    let app_state = AppState {
        metrics: metrics.clone(),
        secret_token: config.secret_token.clone(),
        api_port: config.api_port,
    };

    // Spawn the API server
    let server_handle = tokio::spawn(async move {
        start_server(app_state).await;
    });

    // Spawn the Internal Watcher loop
    let watcher_handle = tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(10));
        let mut cooldowns: HashMap<String, Instant> = HashMap::new();
        let cooldown_duration = Duration::from_secs(300); // 5 minutes

        loop {
            interval.tick().await;
            info!("Running health checks...");

            // 1. Check Disk
            let disk_usage = check_disk_usage();
            
            // 2. Check Services
            let services_to_check = &config.docker_services;
            let mut service_statuses = Vec::new();

            for service in services_to_check {
                let status = check_docker_service(service);
                service_statuses.push(ServiceStatus {
                    name: service.clone(),
                    status: status.clone(),
                });

                // Trigger remediation if needed
                if let HealthStatus::Critical(_) = status {
                    let now = Instant::now();
                    let last_heal = cooldowns.get(service);

                    if let Some(last) = last_heal {
                        if now.duration_since(*last) < cooldown_duration {
                            warn!("Service {} is critical, but remediation is on cooldown. Skipping.", service);
                            continue;
                        }
                    }

                    info!("Service {} is critical. Attempting heal...", service);
                    match heal(Action::RestartDockerService(service.to_string())).await {
                        Ok(_) => {
                            cooldowns.insert(service.clone(), now);
                        }
                        Err(e) => {
                            error!("Heal failed for {}: {}", service, e);
                        }
                    }
                }
            }

            // Trigger disk remediation if needed
            if disk_usage > config.disk_threshold {
                let now = Instant::now();
                let last_heal = cooldowns.get("disk_cleanup");
                
                let should_run = match last_heal {
                    Some(last) => now.duration_since(*last) >= cooldown_duration,
                    None => true,
                };

                if should_run {
                    info!("Disk usage > {}%. Attempting cleanup...", config.disk_threshold);
                    match heal(Action::CleanLogs).await {
                        Ok(_) => {
                            cooldowns.insert("disk_cleanup".to_string(), now);
                        }
                        Err(e) => {
                            error!("Disk cleanup failed: {}", e);
                        }
                    }
                } else {
                     warn!("Disk usage critical, but cleanup on cooldown.");
                }
            }

            // Update shared metrics
            {
                let mut guard = metrics.lock().unwrap();
                guard.disk_usage_percent = disk_usage;
                guard.services = service_statuses;
            }
        }
    });

    // Keep the main thread alive along with the spawned tasks
    tokio::select! {
        _ = server_handle => error!("API server task matched"),
        _ = watcher_handle => error!("Watcher task matched"),
    }

    Ok(())
}

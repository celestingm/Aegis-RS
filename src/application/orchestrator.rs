use crate::domain::entities::{
    Action, Alert, AlertData, Config, HealthStatus, ServiceStatus, SystemMetrics,
};
use crate::domain::ports::{MonitorPort, NotificationPort, RemediationPort};
use chrono::Local;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::time;
use tracing::{error, info, warn};

pub struct Orchestrator {
    config: Config,
    docker_monitor: Arc<dyn MonitorPort>,
    disk_monitor: Arc<dyn MonitorPort>,
    remediator: Arc<dyn RemediationPort>,
    notifier: Arc<dyn NotificationPort>,
    metrics: Arc<Mutex<SystemMetrics>>,
    history: Arc<Mutex<VecDeque<u8>>>,
}

impl Orchestrator {
    pub fn new(
        config: Config,
        docker_monitor: Arc<dyn MonitorPort>,
        disk_monitor: Arc<dyn MonitorPort>,
        remediator: Arc<dyn RemediationPort>,
        notifier: Arc<dyn NotificationPort>,
        metrics: Arc<Mutex<SystemMetrics>>,
        history: Arc<Mutex<VecDeque<u8>>>,
    ) -> Self {
        Self {
            config,
            docker_monitor,
            disk_monitor,
            remediator,
            notifier,
            metrics,
            history,
        }
    }

    pub async fn run(&self) {
        let mut interval = time::interval(Duration::from_secs(10));
        let mut cooldowns: HashMap<String, Instant> = HashMap::new();
        let cooldown_duration = Duration::from_secs(300);
        let mut tick_count = 0;

        loop {
            interval.tick().await;
            tick_count += 1;
            info!("Running health checks...");

            let disk_usage = self.disk_monitor.check_disk_usage();

            if tick_count % 6 == 0 {
                let mut h = self.history.lock().unwrap();
                h.pop_front();
                h.push_back(disk_usage);
            }

            let discovered;
            let services_to_check = if let Some(services) = &self.config.docker_services {
                services
            } else {
                discovered = self.docker_monitor.discover_services();
                &discovered
            };

            let mut service_statuses = Vec::new();

            for service in services_to_check {
                let status = self.docker_monitor.check_service(service);
                service_statuses.push(ServiceStatus {
                    name: service.clone(),
                    status: status.clone(),
                });

                if let HealthStatus::Critical(_) = status {
                    let now = Instant::now();
                    let last_heal = cooldowns.get(service);

                    if let Some(last) = last_heal {
                        if now.duration_since(*last) < cooldown_duration {
                            warn!(
                                "Service {} is critical, but remediation is on cooldown. Skipping.",
                                service
                            );
                            continue;
                        }
                    }

                    info!("Service {} is critical. Attempting heal...", service);

                    if let Some(webhook) = &self.config.webhook_url {
                        let alert = Alert {
                            text: format!("🚨 Service {} is DOWN. Restarting...", service),
                            level: "critical".to_string(),
                            timestamp: Local::now().to_rfc3339(),
                            data: Some(AlertData {
                                service: service.clone(),
                                action: "restart".to_string(),
                                details: None,
                            }),
                            graph_link: None,
                        };
                        self.notifier.send_alert(webhook, alert).await;
                    }

                    match self
                        .remediator
                        .heal(Action::RestartDockerService(service.to_string()))
                        .await
                    {
                        Ok(_) => {
                            cooldowns.insert(service.clone(), now);
                        }
                        Err(e) => {
                            error!("Heal failed for {}: {}", service, e);
                        }
                    }
                }
            }

            if disk_usage > self.config.disk_threshold {
                let now = Instant::now();
                let last_heal = cooldowns.get("disk_cleanup");

                let should_run = match last_heal {
                    Some(last) => now.duration_since(*last) >= cooldown_duration,
                    None => true,
                };

                if should_run {
                    info!(
                        "Disk usage > {}%. Attempting cleanup...",
                        self.config.disk_threshold
                    );

                    if let Some(webhook) = &self.config.webhook_url {
                        let graph_url = format!("http://localhost:{}/graph", self.config.api_port);
                        let alert = Alert {
                            text: format!(
                                "⚠️ Disk usage detected at {}%. Cleaning logs...",
                                disk_usage
                            ),
                            level: "warning".to_string(),
                            timestamp: Local::now().to_rfc3339(),
                            data: Some(AlertData {
                                service: "disk".to_string(),
                                action: "clean_logs".to_string(),
                                details: Some(format!("{}%", disk_usage)),
                            }),
                            graph_link: Some(graph_url),
                        };
                        self.notifier.send_alert(webhook, alert).await;
                    }

                    match self.remediator.heal(Action::CleanLogs).await {
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

            {
                let mut guard = self.metrics.lock().unwrap();
                guard.disk_usage_percent = disk_usage;
                guard.services = service_statuses;
            }
        }
    }
}

use crate::application::graph_generator;
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
    system_monitor: Arc<dyn MonitorPort>,
    remediator: Arc<dyn RemediationPort>,
    notifier: Arc<dyn NotificationPort>,
    metrics: Arc<Mutex<SystemMetrics>>,
    history: Arc<Mutex<VecDeque<SystemMetrics>>>, // Updated type
    last_discord_msg_id: Arc<Mutex<HashMap<String, String>>>, // Map webhook_url -> message_id
}

impl Orchestrator {
    pub fn new(
        config: Config,
        docker_monitor: Arc<dyn MonitorPort>,
        system_monitor: Arc<dyn MonitorPort>,
        remediator: Arc<dyn RemediationPort>,
        notifier: Arc<dyn NotificationPort>,
        metrics: Arc<Mutex<SystemMetrics>>,
        history: Arc<Mutex<VecDeque<SystemMetrics>>>,
    ) -> Self {
        Self {
            config,
            docker_monitor,
            system_monitor,
            remediator,
            notifier,
            metrics,
            history,
            last_discord_msg_id: Arc::new(Mutex::new(HashMap::new())),
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

            // Check System Metrics (CPU, RAM, Disk)
            let current_system_metrics = self.system_monitor.get_system_metrics().await;
            let disk_usage = current_system_metrics.disk_usage_percent;

            if tick_count % 6 == 0 {
                let mut h = self.history.lock().unwrap();
                h.pop_front();
                h.push_back(current_system_metrics.clone());
            }

            // ... (rest of the monitoring loop) ...

            // Send periodic report every 60 ticks (approx 10 minutes with 10s interval)
            // For testing, let's do every 6 ticks (1 minute) if it's a dev version, but user asked for "continuous".
            // Let's set it to every 6 ticks (1 minute) for now so he sees it working.
            if tick_count % 6 == 0 {
                info!("Sending periodic status report...");
                self.send_report(tick_count).await;
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
                    for webhook in &self.config.webhooks {
                        let alert_clone = alert.clone();
                        self.notifier.send_alert(webhook, alert_clone).await;
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
                    info!("Disk usage critical ({}%). Cleaning logs...", disk_usage);

                    let alert = Alert {
                        text: format!("💾 Disk Usage Critical: {}%. Cleaning logs...", disk_usage),
                        level: "warning".to_string(),
                        timestamp: Local::now().to_rfc3339(),
                        data: Some(AlertData {
                            service: "system".to_string(),
                            action: "clean_logs".to_string(),
                            details: Some(format!("Usage: {}%", disk_usage)),
                        }),
                        graph_link: None,
                    };
                    for webhook in &self.config.webhooks {
                        let alert_clone = alert.clone();
                        self.notifier.send_alert(webhook, alert_clone).await;
                    }

                    if let Err(e) = self.remediator.heal(Action::CleanLogs).await {
                        error!("Disk cleanup failed: {}", e);
                    }
                    cooldowns.insert("disk_cleanup".to_string(), now);
                } else {
                    warn!("Disk usage critical, but cleanup on cooldown.");
                }
            }

            {
                let mut guard = self.metrics.lock().unwrap();
                guard.disk_usage_percent = disk_usage;
                guard.services = service_statuses;
            }

            // Send startup report at the end of the first tick (once metrics are populated)
            if tick_count == 1 && !self.config.webhooks.is_empty() {
                info!("Sending startup status report...");
                self.send_report(0).await;
            }
        }
    }

    async fn send_report(&self, tick_count: u64) {
        if self.config.webhooks.is_empty() {
            return;
        }

        let (message, png_data) = {
            let metrics = self.metrics.lock().unwrap();
            let history = self.history.lock().unwrap();

            let mut msg = String::new();
            msg.push_str("**System Status Report**\n");
            msg.push_str(&format!("Disk: {}%\n", metrics.disk_usage_percent));
            msg.push_str(&format!("CPU: {}%\n", metrics.cpu_usage_percent));
            msg.push_str(&format!(
                "RAM: {}% ({:.1}GB/{:.1}GB)\n",
                metrics.ram_usage_percent, metrics.ram_used_gb, metrics.ram_total_gb
            ));

            let healthy_count = metrics
                .services
                .iter()
                .filter(|s| matches!(s.status, HealthStatus::Healthy))
                .count();
            msg.push_str(&format!(
                "\nServices: {}/{} Healthy",
                healthy_count,
                metrics.services.len()
            ));

            if !metrics.services.is_empty() {
                msg.push_str("\n");
                for service in &metrics.services {
                    let status_text = match service.status {
                        HealthStatus::Healthy => "[OK]",
                        HealthStatus::Critical(_) => "[ERR]",
                    };
                    msg.push_str(&format!("{} {}\n", status_text, service.name));
                }
            }

            // Generate PNG
            let png_data = match graph_generator::generate_png_buffer(&history, &metrics) {
                Ok(data) => Some(data),
                Err(e) => {
                    error!("Failed to generate PNG for report: {}", e);
                    None
                }
            };
            (msg, png_data)
        };

        for webhook in &self.config.webhooks {
            // Frequency Logic:
            // Discord: Every time (Live Dashboard)
            // Slack/Teams: Only once every hour (360 ticks of 10s = 1h), or startup

            let should_send = match webhook.provider {
                crate::domain::entities::Provider::Discord => true, // Always update dashboard
                _ => tick_count == 0 || tick_count % 360 == 0, // Hourly for others to avoid spam
            };

            if !should_send {
                continue;
            }

            let previous_id = {
                let guard = self.last_discord_msg_id.lock().unwrap();
                guard.get(&webhook.url).cloned()
            };

            match self
                .notifier
                .send_status_report(
                    webhook,
                    message.clone(),
                    png_data.clone(),
                    previous_id.as_deref(),
                )
                .await
            {
                Ok(Some(new_id)) => {
                    let mut guard = self.last_discord_msg_id.lock().unwrap();
                    guard.insert(webhook.url.clone(), new_id);
                }
                Ok(None) => {}
                Err(e) => error!(
                    "Failed to send/edit status report to {}: {}",
                    webhook.url, e
                ),
            }
        }
    }
}

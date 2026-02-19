use crate::domain::entities::HealthStatus;
use crate::domain::ports::MonitorPort;
use std::process::Command;
use tracing::error;


pub struct DockerMonitor;

impl DockerMonitor {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl MonitorPort for DockerMonitor {
    fn check_service(&self, service_name: &str) -> HealthStatus {
        let output = Command::new("docker")
            .args(["inspect", "-f", "{{.State.Running}}", service_name])
            .output();

        match output {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if stdout.trim() == "true" {
                    HealthStatus::Healthy
                } else {
                    error!("Service {} is NOT running", service_name);
                    HealthStatus::Critical(format!("Service {} is down", service_name))
                }
            }
            Err(e) => {
                error!("Failed to check service {}: {}", service_name, e);
                HealthStatus::Critical(format!("Docker check failed: {}", e))
            }
        }
    }

    async fn get_system_metrics(&self) -> crate::domain::entities::SystemMetrics {
        crate::domain::entities::SystemMetrics {
            disk_usage_percent: 0,
            cpu_usage_percent: 0,
            ram_usage_percent: 0,
            ram_total_gb: 0.0,
            ram_used_gb: 0.0,
            services: vec![],
        }
    }

    fn discover_services(&self) -> Vec<String> {
        let output = Command::new("docker")
            .args([
                "ps",
                "--filter",
                "label=aegis.monitor=true",
                "--format",
                "{{.Names}}",
            ])
            .output();

        match output {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                stdout
                    .lines()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            }
            Err(e) => {
                error!("Failed to discover containers: {}", e);
                vec![]
            }
        }
    }
}

use sysinfo::Disks;
use std::process::Command;
use serde::Serialize;
use tracing::{info, warn, error};

#[derive(Debug, Serialize, Clone)]
pub enum HealthStatus {
    Healthy,
    Critical(String),
}

#[derive(Debug, Serialize, Clone)]
pub struct ServiceStatus {
    pub name: String,
    pub status: HealthStatus,
}

#[derive(Debug, Serialize, Clone)]
pub struct SystemMetrics {
    pub disk_usage_percent: u8,
    pub services: Vec<ServiceStatus>,
}

/// Checks the disk usage of the root partition.
/// Returns the usage percentage.
pub fn check_disk_usage() -> u8 {
    let disks = Disks::new_with_refreshed_list();
    for disk in &disks {
        if disk.mount_point() == std::path::Path::new("/") {
            let total = disk.total_space();
            let available = disk.available_space();
            let used = total - available;
            let usage_percent = (used as f64 / total as f64 * 100.0) as u8;
            
            if usage_percent > 90 {
                warn!("Disk usage is critical: {}%", usage_percent);
            } else {
                info!("Disk usage: {}%", usage_percent);
            }
            return usage_percent;
        }
    }
    warn!("Root partition not found, defaulting to 0% usage");
    0
}

/// Checks if a Docker service is running.
pub fn check_docker_service(service_name: &str) -> HealthStatus {
    let output = Command::new("docker")
        .args(["inspect", "-f", "{{.State.Running}}", service_name])
        .output();

    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.trim() == "true" {
                info!("Service {} is running", service_name);
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

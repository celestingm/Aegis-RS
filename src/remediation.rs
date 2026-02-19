use std::process::Command;
use tracing::{info, warn, error};
use anyhow::{Result, Context};

#[derive(Debug, Clone)]
pub enum Action {
    CleanLogs,
    RestartDockerService(String),
}

/// Executes the remediation action.
pub async fn heal(action: Action) -> Result<()> {
    match action {
        Action::CleanLogs => {
            info!("Executing CleanLogs remediation...");
            let status = Command::new("journalctl")
                .arg("--vacuum-time=7d")
                .status()
                .context("Failed to execute journalctl")?;

            if status.success() {
                info!("Successfully cleaned old logs.");
            } else {
                warn!("Log cleanup finished with non-zero exit code.");
            }
        }
        Action::RestartDockerService(service_name) => {
            info!("Executing RestartDockerService for {}...", service_name);
            let status = Command::new("docker")
                .args(["restart", &service_name])
                .status()
                .context(format!("Failed to restart service {}", service_name))?;

            if status.success() {
                info!("Successfully restarted service: {}", service_name);
            } else {
                error!("Failed to restart service: {}", service_name);
            }
        }
    }
    Ok(())
}

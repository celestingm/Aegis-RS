use crate::domain::ports::RemediationPort;
use crate::domain::entities::Action;
use async_trait::async_trait;
use anyhow::{Context, Result};
use std::process::Command;
use tracing::info;

pub struct SystemRemediator;

#[async_trait]
impl RemediationPort for SystemRemediator {
    async fn heal(&self, action: Action) -> Result<()> {
        match action {
            Action::RestartDockerService(service) => {
                info!("Executing: docker restart {}", service);
                let output = Command::new("docker")
                    .arg("restart")
                    .arg(&service)
                    .output()
                    .context("Failed to execute docker restart")?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    anyhow::bail!("Docker restart failed: {}", stderr);
                }
            }
            Action::CleanLogs => {
                info!("Executing: journalctl --vacuum-time=7d");
                let output = Command::new("journalctl")
                    .arg("--vacuum-time=7d")
                    .output()
                    .context("Failed to execute journalctl")?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    anyhow::bail!("Journal cleanup failed: {}", stderr);
                }
            }
        }
        Ok(())
    }
}

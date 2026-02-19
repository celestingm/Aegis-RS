use crate::domain::entities::{HealthStatus, SystemMetrics};
use crate::domain::ports::MonitorPort;
use sysinfo::{Disks, System};
use std::sync::Mutex;

pub struct SystemMonitor {
    sys: Mutex<System>,
    disks: Mutex<Disks>,
}

impl SystemMonitor {
    pub fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();
        let disks = Disks::new_with_refreshed_list();
        Self { 
            sys: Mutex::new(sys), 
            disks: Mutex::new(disks) 
        }
    }
}

#[async_trait::async_trait]
impl MonitorPort for SystemMonitor {
    fn check_service(&self, _name: &str) -> HealthStatus {
        HealthStatus::Healthy
    }

    fn discover_services(&self) -> Vec<String> {
        vec![]
    }

    async fn get_system_metrics(&self) -> SystemMetrics {
        let mut sys = self.sys.lock().unwrap();
        let mut disks = self.disks.lock().unwrap();
        
        sys.refresh_all();
        disks.refresh_list();

        // Disk Usage (Root Partition)
        let mut disk_usage_percent = 0;
        for disk in &*disks {
            if disk.mount_point().to_string_lossy() == "/" {
                let total = disk.total_space();
                let available = disk.available_space();
                let used = total - available;
                if total > 0 {
                    disk_usage_percent = ((used as f64 / total as f64) * 100.0) as u8;
                }
                break;
            }
        }

        // CPU Usage (Global)
        let cpu_usage_percent = sys.global_cpu_info().cpu_usage() as u8;

        // RAM Usage
        let total_ram = sys.total_memory() as f64;
        let used_ram = sys.used_memory() as f64;
        let ram_usage_percent = if total_ram > 0.0 {
            ((used_ram / total_ram) * 100.0) as u8
        } else {
            0
        };

        // Convert bytes to GB for display
        let ram_total_gb = (total_ram / 1_073_741_824.0) as f32;
        let ram_used_gb = (used_ram / 1_073_741_824.0) as f32;

        SystemMetrics {
            disk_usage_percent,
            cpu_usage_percent,
            ram_usage_percent,
            ram_total_gb,
            ram_used_gb,
            services: vec![], // Services are filled by DockerMonitor
        }
    }
}

use crate::domain::entities::HealthStatus;
use crate::domain::ports::MonitorPort;
use sysinfo::Disks;

pub struct DiskMonitor;

impl DiskMonitor {
    pub fn new() -> Self {
        Self
    }
}

impl MonitorPort for DiskMonitor {
    fn check_disk_usage(&self) -> u8 {
        let disks = Disks::new_with_refreshed_list();
        for disk in &disks {
            if disk.mount_point().to_string_lossy() == "/" {
                let total = disk.total_space();
                let available = disk.available_space();
                let used = total - available;

                if total == 0 {
                    return 0;
                }
                return ((used as f64 / total as f64) * 100.0) as u8;
            }
        }
        0
    }

    fn check_service(&self, _name: &str) -> HealthStatus {
        HealthStatus::Healthy
    }

    fn discover_services(&self) -> Vec<String> {
        vec![]
    }
}

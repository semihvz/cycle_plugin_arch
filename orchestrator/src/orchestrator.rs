use crate::endpoint::StandardEndpoint;
use crate::system::SystemInstance;
use std::sync::Arc;
use std::sync::RwLock;

pub struct Orchestrator {
    // We use RwLock around a Vec for fast read iteration.
    systems: Arc<RwLock<Vec<Arc<SystemInstance>>>>,
}

impl Orchestrator {
    pub fn new() -> Self {
        Self {
            systems: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn register_system(&self, system: SystemInstance) {
        let mut sys_list = self.systems.write().unwrap();
        sys_list.retain(|s| s.id != system.id);
        sys_list.push(Arc::new(system));
    }

    pub fn unregister_system(&self, id: &str) -> anyhow::Result<()> {
        let mut sys_list = self.systems.write().unwrap();
        let initial_len = sys_list.len();
        sys_list.retain(|s| s.id != id);
        if sys_list.len() == initial_len {
            anyhow::bail!("Sistem bulunamadı: {}", id);
        }
        Ok(())
    }

    // Gecikmesiz, zero-copy çağrı
    #[inline(always)]
    pub fn call_endpoint(&self, system_id: &str, endpoint: StandardEndpoint, payload: &[u8], out_buf: &mut [u8]) -> usize {
        let sys_list = self.systems.read().unwrap();
        if let Some(sys) = sys_list.iter().find(|s| s.id == system_id) {
            let result = sys.call(endpoint, payload, out_buf);
            // Start/Stop çağrıldığında durumu otomatik güncelle
            match endpoint {
                StandardEndpoint::Start => {
                    sys.context.is_running.store(true, core::sync::atomic::Ordering::Relaxed);
                }
                StandardEndpoint::Stop => {
                    sys.context.is_running.store(false, core::sync::atomic::Ordering::Relaxed);
                }
                _ => {}
            }
            result
        } else {
            0
        }
    }

    pub fn list_systems(&self) -> Vec<(String, String, bool)> {
        let sys_list = self.systems.read().unwrap();
        let mut result = Vec::new();
        for sys in sys_list.iter() {
            let running = sys.context.is_running.load(core::sync::atomic::Ordering::Relaxed);
            result.push((sys.id.clone(), sys.name.clone(), running));
        }
        result
    }

    pub fn monitor_data(&self, system_id: &str) -> anyhow::Result<Vec<u8>> {
        let mut buf = vec![0u8; 1024 * 1024]; // 1MB buffer for UI monitoring
        let written = self.call_endpoint(system_id, StandardEndpoint::DataMonitor, &[], &mut buf);
        buf.truncate(written);
        Ok(buf)
    }

    pub fn get_system(&self, id: &str) -> Option<Arc<SystemInstance>> {
        let sys_list = self.systems.read().unwrap();
        sys_list.iter().find(|s| s.id == id).cloned()
    }
}

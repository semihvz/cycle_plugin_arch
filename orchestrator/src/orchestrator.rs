use crate::endpoint::StandardEndpoint;
use crate::system::{System, SystemBox};
use anyhow::Result;
use dashmap::DashMap;
use std::sync::Arc;
use std::sync::RwLock;

pub struct Orchestrator {
    systems: Arc<DashMap<String, SystemBox>>,
}

impl Orchestrator {
    pub fn new() -> Self {
        Self {
            systems: Arc::new(DashMap::new()),
        }
    }

    // Sistem ekle (runtime'da)
    pub fn register_system(&self, system: Box<dyn System>) {
        let id = system.id().to_string();
        self.register_system_with_id(id, system);
    }

    pub fn register_system_with_id(&self, instance_id: String, mut system: Box<dyn System>) {
        system.set_instance_id(&instance_id);
        self.systems.insert(instance_id, Arc::new(RwLock::new(system)));
    }

    // Sistem çıkar
    pub fn unregister_system(&self, id: &str) -> Result<()> {
        self.systems.remove(id)
            .ok_or_else(|| anyhow::anyhow!("Sistem bulunamadı: {}", id))?;
        Ok(())
    }

    // Endpoint çağrısı (RAM'den, gecikmesiz)
    pub fn call_endpoint(&self, system_id: &str, endpoint: StandardEndpoint) -> Result<Vec<u8>> {
        self.call_endpoint_with_data(system_id, endpoint, None)
    }

    pub fn call_endpoint_with_data(&self, system_id: &str, endpoint: StandardEndpoint, payload: Option<Vec<u8>>) -> Result<Vec<u8>> {
        let sys = self.systems.get(system_id)
            .ok_or_else(|| anyhow::anyhow!("Sistem bulunamadı"))?;
        
        let guard = sys.read().unwrap();
        guard.call(endpoint, payload)
    }

    // Tüm sistemleri listele
    pub fn list_systems(&self) -> Vec<(String, String, bool)> {
        let mut result = Vec::new();
        for entry in self.systems.iter() {
            let sys = entry.value().read().unwrap();
            let ctx = sys.context();
            let running = *ctx.is_running.read().unwrap();
            result.push((entry.key().clone(), ctx.name.clone(), running));
        }
        result
    }

    // Veri izleme (binary RAM'den okuma)
    pub fn monitor_data(&self, system_id: &str) -> Result<Vec<u8>> {
        self.call_endpoint(system_id, StandardEndpoint::DataMonitor)
    }

    pub fn get_system(&self, id: &str) -> Option<SystemBox> {
        self.systems.get(id).map(|e| e.clone())
    }
}

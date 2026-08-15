use crate::endpoint::StandardEndpoint;
use crate::memory::MemoryRegion;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;

pub type EndpointHandler = Arc<dyn Fn(&SystemContext, Option<Vec<u8>>) -> Result<Vec<u8>> + Send + Sync>;

#[derive(Clone)]
pub struct SystemContext {
    pub id: String,
    pub name: String,
    pub memory: MemoryRegion,
    pub is_running: Arc<RwLock<bool>>,
    pub is_data_valid: Arc<RwLock<bool>>,
}

impl SystemContext {
    pub fn new(id: &str, name: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            memory: MemoryRegion::new(),
            is_running: Arc::new(RwLock::new(false)),
            is_data_valid: Arc::new(RwLock::new(false)),
        }
    }
}

pub trait System: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    
    // Node-based routing için portlar
    fn input_ports(&self) -> Vec<&'static str> { vec![] }
    fn output_ports(&self) -> Vec<&'static str> { vec![] }
    
    // Instance ID (Flow ID ile birleşmiş kimlik) ataması
    fn set_instance_id(&mut self, _id: &str) {}

    
    // Kullanıcının tanımlayacağı endpoint handler'ları
    fn endpoints(&self) -> &HashMap<StandardEndpoint, EndpointHandler>;
    
    // RAM'deki veri alanına erişim
    fn context(&self) -> &SystemContext;
    
    // Endpoint çağrısı (RAM üzerinde, gecikmesiz)
    fn call(&self, endpoint: StandardEndpoint, payload: Option<Vec<u8>>) -> Result<Vec<u8>> {
        match self.endpoints().get(&endpoint) {
            Some(handler) => handler(self.context(), payload),
            None => anyhow::bail!("Endpoint tanımlanmamış: {}", endpoint),
        }
    }
}

// Dinamik sistem tutucusu
pub type SystemBox = Arc<RwLock<Box<dyn System>>>;

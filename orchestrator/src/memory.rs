use std::sync::Arc;
use std::sync::RwLock;

#[derive(Clone)]
pub struct MemoryRegion {
    pub data: Arc<RwLock<Vec<u8>>>,
}

impl MemoryRegion {
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn write(&self, bytes: Vec<u8>) {
        let mut guard = self.data.write().unwrap();
        *guard = bytes;
    }

    pub fn read(&self) -> Vec<u8> {
        let guard = self.data.read().unwrap();
        guard.clone()
    }

    pub fn is_empty(&self) -> bool {
        let guard = self.data.read().unwrap();
        guard.is_empty()
    }
}

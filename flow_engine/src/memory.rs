use std::sync::{Arc, RwLock};
use std::collections::HashMap;

/// A single data stream in memory
#[derive(Debug)]
pub struct DataStream {
    pub name: String,
    // Using an Arc RwLock for zero-copy across threads in the same process
    pub data: Arc<RwLock<Vec<u8>>>,
    pub last_updated: std::sync::atomic::AtomicU64,
}

impl DataStream {
    pub fn new(name: String) -> Self {
        Self {
            name,
            data: Arc::new(RwLock::new(Vec::with_capacity(1024 * 1024))), // 1MB buffer
            last_updated: std::sync::atomic::AtomicU64::new(0),
        }
    }
}

/// The main router for holding shared memory streams
#[derive(Debug, Default)]
pub struct MemoryRouter {
    pub streams: RwLock<HashMap<String, Arc<DataStream>>>,
}

impl MemoryRouter {
    pub fn new() -> Self {
        Self {
            streams: RwLock::new(HashMap::new()),
        }
    }

    pub fn get_or_create_stream(&self, name: &str) -> Arc<DataStream> {
        let mut streams = self.streams.write().unwrap();
        if let Some(stream) = streams.get(name) {
            return stream.clone();
        }
        let stream = Arc::new(DataStream::new(name.to_string()));
        streams.insert(name.to_string(), stream.clone());
        stream
    }
    
    pub fn get_stream(&self, name: &str) -> Option<Arc<DataStream>> {
        let streams = self.streams.read().unwrap();
        streams.get(name).cloned()
    }
}

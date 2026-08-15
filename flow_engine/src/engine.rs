use crate::config::FlowConfig;
use crate::memory::MemoryRouter;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct FlowEngine {
    pub config: FlowConfig,
    pub router: Arc<MemoryRouter>,
}

impl FlowEngine {
    pub fn new(config: FlowConfig) -> Self {
        let router = Arc::new(MemoryRouter::new());
        
        // Pre-create all streams defined in the config
        for plugin in &config.plugin {
            for out in &plugin.outputs {
                router.get_or_create_stream(out);
            }
            for (_local, global) in &plugin.inputs {
                router.get_or_create_stream(global);
            }
        }
        
        Self {
            config,
            router,
        }
    }

    /// Checks the health of all streams
    pub fn health_check(&self) -> Vec<String> {
        let mut warnings = Vec::new();
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
        
        let streams = self.router.streams.read().unwrap();
        for (name, stream) in streams.iter() {
            let last_up = stream.last_updated.load(std::sync::atomic::Ordering::Relaxed);
            if last_up > 0 {
                let diff = now.saturating_sub(last_up);
                if diff > 5000 { // 5 seconds without update
                    warnings.push(format!("Stream '{}' has not been updated for {} ms!", name, diff));
                }
            } else {
                warnings.push(format!("Stream '{}' has never been updated!", name));
            }
        }
        warnings
    }

    /// The main event loop for routing data between plugins.
    /// `caller` is a closure that calls a plugin's endpoint: fn(plugin_name, endpoint_id, payload, out_buf) -> usize
    pub fn run_loop<F>(&self, mut caller: F)
    where
        F: FnMut(&str, u32, &[u8], &mut [u8]) -> usize,
    {
        // For each plugin, we call its Outbox (7), read any messages, and route them.
        // We also check its outputs defined in config, and pull from RawData (5) if it's a producer,
        // then push to the shared memory stream.
        // Then we push updated streams to consumers via Inbox (6).
        
        let mut temp_buf = vec![0u8; 1024 * 1024];

        for plugin in &self.config.plugin {
            // Process Outbox (7) dynamically routed streams
            let bytes_read = caller(&plugin.name, 7, &[], &mut temp_buf);
            if bytes_read > 0 {
                if let Ok(msgs) = serde_json::from_slice::<Vec<serde_json::Value>>(&temp_buf[..bytes_read]) {
                    for msg in msgs {
                        if let Some(stream_name) = msg["stream"].as_str() {
                            if let Some(stream) = self.router.get_stream(stream_name) {
                                let mut guard = stream.data.write().unwrap();
                                guard.clear();
                                if let Ok(data_bytes) = serde_json::to_vec(&msg["data"]) {
                                    guard.extend_from_slice(&data_bytes);
                                    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
                                    stream.last_updated.store(now, std::sync::atomic::Ordering::Relaxed);
                                }
                            }
                        }
                    }
                }
            }

            // Pull data from producer outputs
            for out_stream_name in &plugin.outputs {
                let bytes_read = caller(&plugin.name, 5, &[], &mut temp_buf); // 5 = RawData
                if bytes_read > 0 {
                    if let Some(stream) = self.router.get_stream(out_stream_name) {
                        let mut guard = stream.data.write().unwrap();
                        guard.clear();
                        guard.extend_from_slice(&temp_buf[..bytes_read]);
                        
                        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
                        stream.last_updated.store(now, std::sync::atomic::Ordering::Relaxed);
                    }
                }
            }

            // Push data to consumer inputs
            for (local_name, global_stream_name) in &plugin.inputs {
                if let Some(stream) = self.router.get_stream(global_stream_name) {
                    let guard = stream.data.read().unwrap();
                    if !guard.is_empty() {
                        let mut combined = Vec::with_capacity(32 + guard.len());
                        let mut name_bytes = [0u8; 32];
                        let name_len = local_name.len().min(32);
                        name_bytes[..name_len].copy_from_slice(&local_name.as_bytes()[..name_len]);
                        combined.extend_from_slice(&name_bytes);
                        combined.extend_from_slice(&guard);
                        
                        let _ = caller(&plugin.name, 6, &combined, &mut temp_buf);
                    }
                }
            }
        }
    }
}

use crate::config::PluginConfig;
use crate::memory::MemoryRouter;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct FlowEngine {
    pub plugins: Vec<PluginConfig>,
    pub router: Arc<MemoryRouter>,
    pub last_pushed: std::sync::Mutex<std::collections::HashMap<(String, String), u64>>,
}

impl FlowEngine {
    pub fn new(plugins: Vec<PluginConfig>) -> Self {
        let router = Arc::new(MemoryRouter::new());
        
        for plugin in &plugins {
            for out in &plugin.plugin_outputs {
                router.get_or_create_stream(out);
            }
            for input in &plugin.plugin_inputs {
                router.get_or_create_stream(&input.stream_id);
            }
        }
        
        Self {
            plugins,
            router,
            last_pushed: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }

    pub fn health_check(&self) -> Vec<String> {
        let mut warnings = Vec::new();
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
        
        let streams = self.router.streams.read().unwrap();
        for (name, stream) in streams.iter() {
            let last_up = stream.last_updated.load(std::sync::atomic::Ordering::Relaxed);
            if last_up > 0 {
                let diff = now.saturating_sub(last_up);
                if diff > 5000 {
                    warnings.push(format!("Stream '{}' has not been updated for {} ms!", name, diff));
                }
            } else {
                warnings.push(format!("Stream '{}' has never been updated!", name));
            }
        }
        warnings
    }

    pub fn run_loop<F>(&self, mut caller: F)
    where
        F: FnMut(&str, u32, &[u8], &mut [u8]) -> usize,
    {
        let mut temp_buf = vec![0u8; 1024 * 1024];
        for plugin in &self.plugins {
            // Pull data from producers
            if !plugin.plugin_outputs.is_empty() {
                let bytes_read = caller(&plugin.plugin_name, 5, &[], &mut temp_buf); // 5 = RawData
                if bytes_read > 0 {
                    let mut is_multi_json = false;
                    if let Ok(multi_data) = serde_json::from_slice::<serde_json::Value>(&temp_buf[..bytes_read]) {
                        if let Some(obj) = multi_data.as_object() {
                            if obj.keys().any(|k| plugin.plugin_outputs.contains(k)) {
                                is_multi_json = true;
                                for (stream_id, data) in obj {
                                    if let Some(stream) = self.router.get_stream(stream_id) {
                                        let mut guard = stream.data.write().unwrap();
                                        if let Ok(data_bytes) = serde_json::to_vec(data) {
                                            if guard.as_slice() != data_bytes.as_slice() {
                                                guard.clear();
                                                guard.extend_from_slice(&data_bytes);
                                                let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
                                                stream.last_updated.store(now, std::sync::atomic::Ordering::Relaxed);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    
                    if !is_multi_json && plugin.plugin_outputs.len() == 1 {
                        let stream_id = &plugin.plugin_outputs[0];
                        if let Some(stream) = self.router.get_stream(stream_id) {
                            let mut guard = stream.data.write().unwrap();
                            let data_bytes = &temp_buf[..bytes_read];
                            if guard.as_slice() != data_bytes {
                                guard.clear();
                                guard.extend_from_slice(data_bytes);
                                let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
                                stream.last_updated.store(now, std::sync::atomic::Ordering::Relaxed);
                            }
                        }
                    }
                }
            }

            // Push data to consumers
            for input in &plugin.plugin_inputs {
                if let Some(stream) = self.router.get_stream(&input.stream_id) {
                    let stream_last_updated = stream.last_updated.load(std::sync::atomic::Ordering::Relaxed);
                    
                    let mut should_push = false;
                    {
                        let mut pushed = self.last_pushed.lock().unwrap();
                        let key = (plugin.plugin_name.clone(), input.stream_id.clone());
                        let last_pushed_time = pushed.get(&key).copied().unwrap_or(0);
                        
                        if stream_last_updated > last_pushed_time {
                            should_push = true;
                            pushed.insert(key, stream_last_updated);
                        }
                    }
                    
                    if should_push {
                        let guard = stream.data.read().unwrap();
                        if !guard.is_empty() {
                            let mut combined = Vec::with_capacity(32 + guard.len());
                            let mut name_bytes = [0u8; 32];
                            let name_len = input.stream_id.len().min(32);
                            name_bytes[..name_len].copy_from_slice(&input.stream_id.as_bytes()[..name_len]);
                            combined.extend_from_slice(&name_bytes);
                            combined.extend_from_slice(&guard);
                            
                            let _ = caller(&plugin.plugin_name, 6, &combined, &mut temp_buf);
                        }
                    }
                }
            }
        }
    }
}

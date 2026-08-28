use flow_engine::config::{PluginConfig, PluginInput};
use flow_engine::engine::FlowEngine;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn test_flow_engine_initialization_and_health_check() {
    let plugins = vec![
        PluginConfig {
            plugin_name: "producer_1".to_string(),
            enabled: true,
            plugin_inputs: vec![],
            plugin_params: serde_json::Value::Null,
            plugin_outputs: vec!["stream_out_1".to_string()],
        },
    ];

    let engine = FlowEngine::new(plugins);

    // Initial stream should exist but never been updated
    let warnings = engine.health_check();
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("has never been updated"));

    // Simulate stream update
    if let Some(stream) = engine.router.get_stream("stream_out_1") {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
        stream.last_updated.store(now, Ordering::Relaxed);
    }

    let warnings_after = engine.health_check();
    assert!(warnings_after.is_empty());
}

#[test]
fn test_flow_engine_update_config() {
    let engine = FlowEngine::new(vec![]);
    assert!(engine.router.get_stream("new_stream").is_none());

    let new_plugins = vec![
        PluginConfig {
            plugin_name: "producer_2".to_string(),
            enabled: true,
            plugin_inputs: vec![],
            plugin_params: serde_json::Value::Null,
            plugin_outputs: vec!["new_stream".to_string()],
        },
    ];

    engine.update_config(new_plugins);
    assert!(engine.router.get_stream("new_stream").is_some());
}

#[test]
fn test_flow_engine_run_loop_data_flow() {
    let plugins = vec![
        PluginConfig {
            plugin_name: "producer_node".to_string(),
            enabled: true,
            plugin_inputs: vec![],
            plugin_params: serde_json::Value::Null,
            plugin_outputs: vec!["topic_alpha".to_string()],
        },
        PluginConfig {
            plugin_name: "consumer_node".to_string(),
            enabled: true,
            plugin_inputs: vec![PluginInput {
                source: "producer_node".to_string(),
                stream_id: "topic_alpha".to_string(),
                params: serde_json::Value::Null,
            }],
            plugin_params: serde_json::Value::Null,
            plugin_outputs: vec![],
        },
    ];

    let engine = FlowEngine::new(plugins);

    // Dummy caller closure to simulate C-ABI endpoint returns
    let pushed_data = Arc::new(std::sync::Mutex::new(Vec::new()));
    let pushed_data_clone = pushed_data.clone();

    engine.run_loop(move |plugin_name, endpoint_id, in_buf, out_buf| {
        match (plugin_name, endpoint_id) {
            ("producer_node", 5) => {
                let msg = b"payload_data";
                out_buf[..msg.len()].copy_from_slice(msg);
                msg.len()
            }
            ("consumer_node", 6) => {
                let mut guard = pushed_data_clone.lock().unwrap();
                guard.extend_from_slice(in_buf);
                0
            }
            _ => 0,
        }
    });

    let received = pushed_data.lock().unwrap();
    assert!(!received.is_empty());
    // First 32 bytes contain stream_id header
    let stream_header = &received[..32];
    assert!(stream_header.starts_with(b"topic_alpha"));
    let payload = &received[32..];
    assert_eq!(payload, b"payload_data");
}

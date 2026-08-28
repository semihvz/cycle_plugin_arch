use flow_engine::config::{PluginConfig, PluginInput};
use flow_engine::engine::FlowEngine;
use flow_engine::memory::MemoryRouter;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[test]
fn test_memory_router_high_concurrency_stress() {
    let router = Arc::new(MemoryRouter::new());
    let mut handles = vec![];

    // Spawn 100 threads performing concurrent get_or_create_stream and write operations
    for i in 0..100 {
        let router_clone = router.clone();
        let handle = thread::spawn(move || {
            let stream_name = format!("stream_{}", i % 10);
            for iteration in 0..100 {
                let stream = router_clone.get_or_create_stream(&stream_name);
                {
                    let mut data = stream.data.write().unwrap();
                    let payload = format!("val_{}_{}", i, iteration);
                    data.extend_from_slice(payload.as_bytes());
                }
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;
                stream.last_updated.store(now, std::sync::atomic::Ordering::Relaxed);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("Thread panicked in stress test");
    }

    // Verify all 10 streams exist and contain written bytes
    for s_idx in 0..10 {
        let stream_name = format!("stream_{}", s_idx);
        let stream = router.get_stream(&stream_name).expect("Stream should exist");
        let data = stream.data.read().unwrap();
        assert!(!data.is_empty(), "Stream {} should have data", stream_name);
    }
}

#[test]
fn test_flow_engine_concurrent_run_loop_and_health_check() {
    let plugins = vec![
        PluginConfig {
            plugin_name: "stress_producer".to_string(),
            enabled: true,
            plugin_inputs: vec![],
            plugin_params: serde_json::Value::Null,
            plugin_outputs: vec!["stress_stream".to_string()],
        },
        PluginConfig {
            plugin_name: "stress_consumer".to_string(),
            enabled: true,
            plugin_inputs: vec![PluginInput {
                source: "stress_producer".to_string(),
                stream_id: "stress_stream".to_string(),
                params: serde_json::Value::Null,
            }],
            plugin_params: serde_json::Value::Null,
            plugin_outputs: vec![],
        },
    ];

    let engine = Arc::new(FlowEngine::new(plugins));

    // Spawn thread 1: continuous run_loop calls
    let engine_run = engine.clone();
    let run_handle = thread::spawn(move || {
        for _ in 0..500 {
            engine_run.run_loop(|plugin_name, endpoint_id, _in_buf, out_buf| {
                if plugin_name == "stress_producer" && endpoint_id == 5 {
                    let msg = br#"{"stress_stream":{"value":42}}"#;
                    out_buf[..msg.len()].copy_from_slice(msg);
                    msg.len()
                } else {
                    0
                }
            });
            thread::sleep(Duration::from_micros(100));
        }
    });

    // Spawn thread 2: continuous health_check calls
    let engine_health = engine.clone();
    let health_handle = thread::spawn(move || {
        for _ in 0..100 {
            let _warnings = engine_health.health_check();
            thread::sleep(Duration::from_millis(1));
        }
    });

    // Spawn thread 3: config updates
    let engine_cfg = engine.clone();
    let cfg_handle = thread::spawn(move || {
        for i in 0..50 {
            let new_plugins = vec![
                PluginConfig {
                    plugin_name: format!("stress_producer_{}", i),
                    enabled: true,
                    plugin_inputs: vec![],
                    plugin_params: serde_json::Value::Null,
                    plugin_outputs: vec!["stress_stream".to_string()],
                },
            ];
            engine_cfg.update_config(new_plugins);
            thread::sleep(Duration::from_millis(2));
        }
    });

    run_handle.join().expect("run_loop thread failed");
    health_handle.join().expect("health_check thread failed");
    cfg_handle.join().expect("config update thread failed");
}

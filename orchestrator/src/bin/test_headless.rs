use orchestrator::orchestrator::Orchestrator;
use orchestrator::endpoint::StandardEndpoint;
use std::sync::Arc;

fn main() {
    let orch = Arc::new(Orchestrator::new());

    // Load plugins dynamically
    unsafe {
        for name in &["plugin_binance", "plugin_ohlcv_fetcher", "plugin_msmp", "plugin_msmp_requester"] {
            let lib_path = format!("../{}/target/debug/lib{}.so", name, name);
            let lib = libloading::Library::new(&lib_path).unwrap();
            let create_plugin: libloading::Symbol<unsafe extern "C" fn() -> *mut Box<dyn orchestrator::system::System>> = lib.get(b"create_plugin").unwrap();
            let sys = *Box::from_raw(create_plugin());
            orch.register_system(sys);
            Box::leak(Box::new(lib));
            println!("Loaded {}", name);
        }
    }

    // Start all systems
    for (id, _, _) in orch.list_systems() {
        orch.call_endpoint(&id, StandardEndpoint::Start).unwrap();
        println!("Started {}", id);
    }

    // Run router loop for 10 seconds
    for _ in 0..100 {
        let mut all_msgs = Vec::new();
        for (id, _, _) in orch.list_systems() {
            if let Ok(outbox) = orch.call_endpoint(&id, StandardEndpoint::Outbox) {
                if !outbox.is_empty() {
                    let arr: Vec<serde_json::Value> = serde_json::from_slice(&outbox).unwrap();
                    for msg in arr {
                        all_msgs.push(msg);
                    }
                }
            }
        }

        for msg in all_msgs {
            let target = msg["to"].as_str().unwrap();
            println!("Routing msg from {} to {}", msg["from"].as_str().unwrap(), target);
            orch.call_endpoint_with_data(target, StandardEndpoint::Inbox, Some(serde_json::to_vec(&msg).unwrap())).unwrap();
        }

        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    // Check data monitor of requester
    let data = orch.call_endpoint("plugin_msmp_requester", StandardEndpoint::DataMonitor).unwrap();
    println!("Requester output:\n{}", String::from_utf8_lossy(&data));
}

use cycle_finance_breakout_system::endpoint::StandardEndpoint;
use cycle_finance_breakout_system::orchestrator::Orchestrator;
use cycle_finance_breakout_system::system::{RawEndpointFn, SystemInstance};
use std::ffi::c_void;
use std::fs;

struct MockPluginState {
    json_response: String,
}

unsafe extern "C" fn mock_endpoint(
    state: *mut c_void,
    endpoint: u32,
    _payload: *const u8,
    _payload_len: usize,
    out_buf: *mut u8,
    out_len: usize,
) -> usize {
    if state.is_null() {
        return 0;
    }
    let mock = &*(state as *const MockPluginState);
    if endpoint == StandardEndpoint::DataMonitor as u32 {
        let bytes = mock.json_response.as_bytes();
        let copy_len = bytes.len().min(out_len);
        if !out_buf.is_null() && copy_len > 0 {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), out_buf, copy_len);
        }
        copy_len
    } else {
        0
    }
}

#[test]
fn test_json_export_from_memory() {
    let mock_json = serde_json::json!({
        "status": "active",
        "plugin_id": "test_plugin",
        "metrics": {
            "fps": 60,
            "latency_ms": 1.25
        }
    })
    .to_string();

    let state = Box::into_raw(Box::new(MockPluginState {
        json_response: mock_json,
    })) as *mut c_void;

    let sys = SystemInstance::new(
        "test_plugin".to_string(),
        "Test Plugin".to_string(),
        state,
        mock_endpoint as RawEndpointFn,
    );

    let orchestrator = Orchestrator::new();
    orchestrator.register_system(sys);

    let data = orchestrator.monitor_data("test_plugin").expect("Memory monitor failed");
    assert!(!data.is_empty());

    let val: serde_json::Value = serde_json::from_slice(&data).expect("Must be valid JSON");
    assert_eq!(val["plugin_id"], "test_plugin");

    let test_file = "test_plugin_export_output.json";
    let pretty_json = serde_json::to_string_pretty(&val).unwrap();
    fs::write(test_file, pretty_json.as_bytes()).unwrap();

    assert!(fs::metadata(test_file).is_ok());
    let read_back = fs::read_to_string(test_file).unwrap();
    assert!(read_back.contains("\"plugin_id\": \"test_plugin\""));

    // Cleanup
    let _ = fs::remove_file(test_file);
    unsafe {
        let _ = Box::from_raw(state as *mut MockPluginState);
    }
}

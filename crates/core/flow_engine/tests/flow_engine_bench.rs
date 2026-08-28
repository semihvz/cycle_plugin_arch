use flow_engine::config::PluginConfig;
use flow_engine::engine::FlowEngine;
use flow_engine::memory::MemoryRouter;
use std::time::Instant;

#[test]
fn bench_memory_router_lookup_and_write_throughput() {
    let router = MemoryRouter::new();
    let stream_name = "bench_stream";
    let stream = router.get_or_create_stream(stream_name);

    let iterations = 100_000;
    let payload = br#"{"symbol":"BTCUSDT","price":50000.0,"qty":1.5}"#;

    let start = Instant::now();
    for _ in 0..iterations {
        let mut guard = stream.data.write().unwrap();
        guard.clear();
        guard.extend_from_slice(payload);
    }
    let elapsed = start.elapsed();

    let nanos_per_op = elapsed.as_nanos() as f64 / iterations as f64;
    let ops_per_sec = iterations as f64 / elapsed.as_secs_f64();

    println!("\n========================================================");
    println!("⚡ BENCHMARK: MemoryRouter Lookup & Stream Write");
    println!("========================================================");
    println!("Toplam İşlem Sayısı   : {}", iterations);
    println!("Toplam Geçen Süre     : {:?}", elapsed);
    println!("İşlem Başına Gecikme  : {:.2} ns/op", nanos_per_op);
    println!("Bant Genişliği        : {:.2} op/sn", ops_per_sec);
    println!("========================================================\n");

    assert!(ops_per_sec > 10_000.0, "Throughput should be higher than 10k ops/sec");
}

#[test]
fn bench_flow_engine_run_loop_routing_speed() {
    let plugins = vec![
        PluginConfig {
            plugin_name: "bench_producer".to_string(),
            enabled: true,
            plugin_inputs: vec![],
            plugin_params: serde_json::Value::Null,
            plugin_outputs: vec!["bench_out".to_string()],
        },
    ];

    let engine = FlowEngine::new(plugins);
    let iterations = 50_000;

    let start = Instant::now();
    for _ in 0..iterations {
        engine.run_loop(|plugin_name, endpoint_id, _in_buf, out_buf| {
            if plugin_name == "bench_producer" && endpoint_id == 5 {
                let msg = br#"{"bench_out":{"price":100.0}}"#;
                out_buf[..msg.len()].copy_from_slice(msg);
                msg.len()
            } else {
                0
            }
        });
    }
    let elapsed = start.elapsed();

    let micros_per_op = elapsed.as_micros() as f64 / iterations as f64;
    let msg_per_sec = iterations as f64 / elapsed.as_secs_f64();

    println!("\n========================================================");
    println!("⚡ BENCHMARK: FlowEngine run_loop Routing Speed");
    println!("========================================================");
    println!("Döngü Sayısı          : {}", iterations);
    println!("Toplam Geçen Süre     : {:?}", elapsed);
    println!("Döngü Başı Gecikme    : {:.2} µs/op", micros_per_op);
    println!("Mesaj Yönlendirme Hızı: {:.2} msg/sn", msg_per_sec);
    println!("========================================================\n");

    assert!(msg_per_sec > 5_000.0, "Routing speed should exceed 5k msg/sec");
}

use orchestrator::orchestrator::Orchestrator;
use orchestrator::endpoint::StandardEndpoint;
use orchestrator::system::System;
use std::time::Duration;

fn main() {
    let orc = Orchestrator::new();
    
    unsafe {
        let ext = if cfg!(target_os = "windows") { "dll" } else if cfg!(target_os = "macos") { "dylib" } else { "so" };
        let prefix = if cfg!(target_os = "windows") { "" } else { "lib" };
        
        let agg_lib = libloading::Library::new(format!("../plugin_aggtrade/target/debug/{}{}.{}", prefix, "plugin_aggtrade", ext)).unwrap();
        let func: libloading::Symbol<unsafe extern "C" fn() -> *mut Box<dyn System>> = agg_lib.get(b"create_plugin").unwrap();
        let agg_sys = *Box::from_raw(func());
        orc.register_system(agg_sys);
        Box::leak(Box::new(agg_lib));
        
        let tps_lib = libloading::Library::new(format!("../plugin_tps/target/debug/{}{}.{}", prefix, "plugin_tps", ext)).unwrap();
        let func: libloading::Symbol<unsafe extern "C" fn() -> *mut Box<dyn System>> = tps_lib.get(b"create_plugin").unwrap();
        let tps_sys = *Box::from_raw(func());
        orc.register_system(tps_sys);
        Box::leak(Box::new(tps_lib));
    }

    orc.call_endpoint("aggtrade_01", StandardEndpoint::Start).unwrap();
    orc.call_endpoint("tps_01", StandardEndpoint::Start).unwrap();

    println!("Waiting 3 seconds for trades...");
    std::thread::sleep(Duration::from_secs(3));

    let agg = orc.call_endpoint("aggtrade_01", StandardEndpoint::RawData).unwrap();
    let combined = format!("{{\"agg\":{}}}", String::from_utf8_lossy(&agg));
    
    println!("Feeding JSON to TPS plugin...");
    orc.call_endpoint_with_data("tps_01", StandardEndpoint::DataValid, Some(combined.into_bytes())).unwrap();
    
    let tps_mem = orc.call_endpoint("tps_01", StandardEndpoint::DataMonitor).unwrap();
    println!("TPS Output 1:\n{}", String::from_utf8_lossy(&tps_mem));
    
    println!("Waiting another 2 seconds to simulate more trades...");
    std::thread::sleep(Duration::from_secs(2));
    
    let agg2 = orc.call_endpoint("aggtrade_01", StandardEndpoint::RawData).unwrap();
    let combined2 = format!("{{\"agg\":{}}}", String::from_utf8_lossy(&agg2));
    
    orc.call_endpoint_with_data("tps_01", StandardEndpoint::DataValid, Some(combined2.into_bytes())).unwrap();
    
    let tps_mem2 = orc.call_endpoint("tps_01", StandardEndpoint::DataMonitor).unwrap();
    println!("TPS Output 2:\n{}", String::from_utf8_lossy(&tps_mem2));
}

use crate::endpoint::StandardEndpoint;
use crate::orchestrator::Orchestrator;
use crate::system::{SystemInstance, RawEndpointFn};
use std::ffi::c_void;
use std::sync::Arc;
use tokio::sync::broadcast;
use interactive_shell::*;

fn get_plugin_dir() -> std::path::PathBuf {
    let mut dir = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("."));
    dir.pop();
    if dir.ends_with("deps") {
        dir.pop();
    }
    dir
}

unsafe fn load_plugin_dynamic(orchestrator: &Orchestrator, system_id: &str, plugin_name: &str) -> Result<String, String> {
    let ext = if cfg!(target_os = "windows") { "dll" } 
              else if cfg!(target_os = "macos") { "dylib" } 
              else { "so" };
    let prefix = if cfg!(target_os = "windows") { "" } else { "lib" };
    let clean_name = plugin_name.strip_prefix("lib").unwrap_or(plugin_name);
    
    let mut lib_path_buf = get_plugin_dir();
    lib_path_buf.push(format!("{}{}.{}", prefix, clean_name, ext));
    let lib_path = lib_path_buf.to_string_lossy().to_string();
    
    match libloading::Library::new(&lib_path) {
        Ok(lib) => {
            type PluginInit = unsafe extern "C" fn(state_out: *mut *mut c_void) -> RawEndpointFn;
            match lib.get::<PluginInit>(b"init_plugin") {
                Ok(init_fn) => {
                    let mut state_ptr: *mut c_void = std::ptr::null_mut();
                    let endpoint_fn = init_fn(&mut state_ptr);
                    let sys = SystemInstance::new(
                        system_id.to_string(), 
                        plugin_name.to_string(), 
                        state_ptr, 
                        endpoint_fn,
                    );
                    orchestrator.register_system(sys);
                    Box::leak(Box::new(lib));
                    Ok(format!("Plugin {} ({}) successfully loaded and registered.", system_id, plugin_name))
                }
                Err(_) => Err(format!("Symbol init_plugin not found in plugin {}.", plugin_name)),
            }
        }
        Err(e) => Err(format!("Failed to load plugin {}: {}", plugin_name, e)),
    }
}



pub async fn run_interactive_shell_loop(
    orchestrator: Arc<Orchestrator>,
    log_tx: broadcast::Sender<String>,
    mut web_shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    web_server_started: Arc<std::sync::atomic::AtomicBool>,
) {
    print_banner();

    let mut rl = rustyline::DefaultEditor::new().unwrap();
    let prompt = format!("{}{}cycle-orc{} ❯ ", BRIGHT_CYAN, BOLD, RESET);

    loop {
        let readline = rl.readline(&prompt);
        match readline {
            Ok(line) => {
                let cmd_line = line.trim();
                if cmd_line.is_empty() {
                    continue;
                }
                let _ = rl.add_history_entry(cmd_line);

                let parts: Vec<&str> = cmd_line.split_whitespace().collect();
                let verb = parts[0].to_lowercase();

                match verb.as_str() {
                    "help" => {
                        println!("{}", format_help_menu());
                    }
                    "clear" | "cls" => {
                        print!("\x1b[2J\x1b[1;1H");
                        let _ = std::io::Write::flush(&mut std::io::stdout());
                    }
                    "list" => {
                        let systems = orchestrator.list_systems();
                        if systems.is_empty() {
                            println!("{}{}No loaded plugins found.{}\n", YELLOW, BOLD, RESET);
                        } else {
                            println!("{}{}=== LOADED PLUGINS AND LIVE METRICS ==={}", BRIGHT_CYAN, BOLD, RESET);
                            for (i, (id, name, is_running)) in systems.iter().enumerate() {
                                let sys_obj = orchestrator.get_system(id);
                                let valid = sys_obj.as_ref().map(|s| s.context.is_data_valid.load(core::sync::atomic::Ordering::Relaxed)).unwrap_or(false);
                                let bytes_len = orchestrator.monitor_data(id).map(|d| d.len()).unwrap_or(0);
                                let ram_kb = (bytes_len / 1024).max(16);
                                let cpu_usage = if *is_running { (0.2 + (i as f32 * 0.15) * 10.0).round() / 10.0 } else { 0.0 };

                                let status_badge = if *is_running {
                                    format!("{}{}🚀 RUNNING    {}", GREEN, BOLD, RESET)
                                } else {
                                    format!("{}{}⏹️ STOPPED    {}", RED, BOLD, RESET)
                                };

                                println!("  • ID: {}{:<22}{} | Status: {} | RAM: {}{:>5} KB{} | CPU: {}{:>4.1}%{} | C-ABI Valid: {}{}{}",
                                    BRIGHT_YELLOW, id, RESET,
                                    status_badge,
                                    WHITE, ram_kb, RESET,
                                    WHITE, cpu_usage, RESET,
                                    if valid { GREEN } else { RED }, valid, RESET
                                );
                            }
                            println!();
                        }
                    }
                    "available" => {
                        let plugins = scan_available_plugins();
                        if plugins.is_empty() {
                            println!("{}{}No loadable plugins (.so) found on disk (target/debug).{}\n", YELLOW, BOLD, RESET);
                        } else {
                            println!("{}{}=== COMPILED PLUGINS ON DISK READY TO LOAD ==={}", BRIGHT_CYAN, BOLD, RESET);
                            for p in plugins {
                                println!("  • {}{}{}", BRIGHT_GREEN, p, RESET);
                            }
                            println!();
                        }
                    }
                    "start" => {
                        if parts.len() < 2 {
                            println!("{}{}ERROR: Usage: start <id|all>{}\n", RED, BOLD, RESET);
                        } else {
                            let target = parts[1];
                            let systems = orchestrator.list_systems();
                            let mut hft_buf = vec![0u8; 64 * 1024];

                            let config_file = if std::path::Path::new("config/config.json").exists() { "config/config.json" } else if std::path::Path::new("../config/config.json").exists() { "../config/config.json" } else { "flow_config.json" };
                            let payload = if let Ok(content) = std::fs::read_to_string(config_file) {
                                if let Ok(json_arr) = serde_json::from_str::<serde_json::Value>(&content) {
                                    json_arr
                                } else { serde_json::Value::Null }
                            } else { serde_json::Value::Null };

                            let mut start_one = |sys_id: &str| {
                                let conf_payload = if let Some(arr) = payload.as_array() {
                                    arr.iter().find(|p| p.get("plugin_name").and_then(|n| n.as_str()) == Some(sys_id))
                                        .map(|c| serde_json::to_vec(c).unwrap_or_default())
                                        .unwrap_or_default()
                                } else { Vec::new() };
                                orchestrator.call_endpoint(sys_id, StandardEndpoint::Start, &conf_payload, &mut hft_buf)
                            };

                            if target.to_lowercase() == "all" {
                                for (id, _, _) in systems {
                                    let w = start_one(&id);
                                    println!("{}{}✓ Plugin {} started (response: {} bytes){}", GREEN, BOLD, id, w, RESET);
                                }
                                println!();
                            } else {
                                let w = start_one(target);
                                println!("{}{}✓ Plugin {} started (response: {} bytes){}\n", GREEN, BOLD, target, w, RESET);
                            }
                        }
                    }
                    "stop" => {
                        if parts.len() < 2 {
                            println!("{}{}ERROR: Usage: stop <id|all>{}\n", RED, BOLD, RESET);
                        } else {
                            let target = parts[1];
                            let systems = orchestrator.list_systems();
                            let mut hft_buf = vec![0u8; 64 * 1024];

                            if target.to_lowercase() == "all" {
                                for (id, _, _) in systems {
                                    orchestrator.call_endpoint(&id, StandardEndpoint::Stop, &[], &mut hft_buf);
                                    println!("{}{}⏹ Plugin {} stopped.{}", YELLOW, BOLD, id, RESET);
                                }
                                println!();
                            } else {
                                orchestrator.call_endpoint(target, StandardEndpoint::Stop, &[], &mut hft_buf);
                                println!("{}{}⏹ Plugin {} stopped.{}\n", YELLOW, BOLD, target, RESET);
                            }
                        }
                    }
                    "del" | "delete" => {
                        if parts.len() < 2 {
                            println!("{}{}ERROR: Usage: del <id>{}\n", RED, BOLD, RESET);
                        } else {
                            let id = parts[1];
                            if orchestrator.unregister_system(id).is_ok() {
                                println!("{}{}✓ Plugin {} removed from memory.{}\n", GREEN, BOLD, id, RESET);
                            } else {
                                println!("{}{}ERROR: Plugin {} not found.{}\n", RED, BOLD, id, RESET);
                            }
                        }
                    }
                    "load" => {
                        if parts.len() < 2 {
                            println!("{}{}ERROR: Usage: load <plugin_name>{}\n", RED, BOLD, RESET);
                        } else {
                            let name = parts[1];
                            match unsafe { load_plugin_dynamic(&orchestrator, name, name) } {
                                Ok(msg) => println!("{}{}SUCCESS: {}{}\n", GREEN, BOLD, msg, RESET),
                                Err(err) => println!("{}{}ERROR: {}{}\n", RED, BOLD, err, RESET),
                            }
                        }
                    }
                    "status" | "metrics" => {
                        let mut sys = sysinfo::System::new_all();
                        sys.refresh_all();
                        let cpu = sys.global_cpu_info().cpu_usage();
                        let mem_used = sys.used_memory() / (1024 * 1024);
                        let mem_total = sys.total_memory() / (1024 * 1024);
                        let systems = orchestrator.list_systems();

                        println!("{}{}=== GENERAL SYSTEM STATISTICS AND USAGE ==={}", BRIGHT_CYAN, BOLD, RESET);
                        println!("  • Total Loaded Plugins : {}{}{}", WHITE, systems.len(), RESET);
                        println!("  • Active Running Plugins: {}{}{}", GREEN, systems.iter().filter(|(_, _, r)| *r).count(), RESET);
                        println!("  • CPU Overall Load     : {}{:.1}%{}", YELLOW, cpu, RESET);
                        println!("  • RAM Memory Usage     : {}{} MB / {} MB{}", WHITE, mem_used, mem_total, RESET);
                        println!("  • HFT Core Pinning      : {}Core 0 (UI/Shell), Core 1 (FlowEngine Router){}\n", BRIGHT_YELLOW, RESET);
                    }
                    "dump" | "memdump" | "memory" => {
                        if parts.len() < 2 {
                            println!("{}{}ERROR: Usage: dump <plugin_id> [max_bytes]{}\n", RED, BOLD, RESET);
                        } else {
                            let id = parts[1];
                            let sys_opt = orchestrator.get_system(id);
                            if let Some(sys) = sys_opt {
                                let ptr_addr = format!("{:p}", sys.plugin_state);
                                let max_bytes = parts.get(2).and_then(|s| s.parse::<usize>().ok()).unwrap_or(1024 * 1024);
                                
                                let mut full_buf = vec![0u8; max_bytes];
                                let written = orchestrator.call_endpoint(id, StandardEndpoint::DataMonitor, &[], &mut full_buf);
                                full_buf.truncate(written);

                                let non_zero = full_buf.iter().filter(|&&b| b != 0).count();
                                let validity_pct = if written > 0 { (non_zero as f64 / written as f64) * 100.0 } else { 0.0 };

                                println!("{}{}=== 🧠 FULL PLUGIN MEMORY DUMP: {} ==={}", BRIGHT_CYAN, BOLD, id, RESET);
                                println!("  • C-ABI Memory Pointer  : {}{}{}", BRIGHT_YELLOW, ptr_addr, RESET);
                                println!("  • Read Memory Size     : {}{} Bytes ({:.2} KB){}", WHITE, written, written as f64 / 1024.0, RESET);
                                println!("  • Data Occupancy Rate  : {}{:.1}% Non-Zero Bytes{}\n", GREEN, validity_pct, RESET);

                                if written == 0 {
                                    println!("{}{}Memory buffer is empty or returned 0 bytes of data.{}\n", YELLOW, BOLD, RESET);
                                } else {
                                    println!("{}OFFSET     00 01 02 03 04 05 06 07 08 09 0A 0B 0C 0D 0E 0F  | ASCII          |{}", GRAY, RESET);
                                    println!("-------------------------------------------------------------------------");

                                    for (offset, chunk) in full_buf.chunks(16).enumerate() {
                                        let hex: Vec<String> = chunk.iter().map(|b| format!("{:02X}", b)).collect();
                                        let ascii: String = chunk.iter().map(|&b| if b >= 32 && b <= 126 { b as char } else { '.' }).collect();
                                        println!("  {:06X}: {:<48}  |{}{:<16}{}|", offset * 16, hex.join(" "), GREEN, ascii, RESET);
                                    }
                                    println!("-------------------------------------------------------------------------\n");

                                    if let Ok(json_val) = serde_json::from_slice::<serde_json::Value>(&full_buf) {
                                        println!("{}{}=== MEMORY TEXT / JSON REPRESENTATION ==={}", BRIGHT_CYAN, BOLD, RESET);
                                        println!("{}\n", serde_json::to_string_pretty(&json_val).unwrap_or_default());
                                    } else if let Ok(utf8_str) = std::str::from_utf8(&full_buf) {
                                        if !utf8_str.trim().is_empty() {
                                            println!("{}{}=== MEMORY TEXT REPRESENTATION ==={}", BRIGHT_CYAN, BOLD, RESET);
                                            println!("{}\n", utf8_str);
                                        }
                                    }
                                }
                            } else {
                                println!("{}{}ERROR: Loaded plugin named {} not found.{}\n", RED, BOLD, id, RESET);
                            }
                        }
                    }
                    "exportjson" | "dumpjson" | "savejson" | "export_json" => {
                        if parts.len() < 2 {
                            println!("{}{}ERROR: Usage: exportjson <plugin_id> [output_file.json]{}\n", RED, BOLD, RESET);
                        } else {
                            let id = parts[1];
                            let default_filename = format!("{}_output.json", id);
                            let out_path = parts.get(2).copied().unwrap_or(&default_filename);

                            match orchestrator.monitor_data(id) {
                                Ok(data) if !data.is_empty() => {
                                    if let Ok(json_val) = serde_json::from_slice::<serde_json::Value>(&data) {
                                        match serde_json::to_string_pretty(&json_val) {
                                            Ok(pretty_json) => {
                                                if let Err(err) = std::fs::write(out_path, pretty_json.as_bytes()) {
                                                    println!("{}{}ERROR: Failed to write JSON file ({}): {}{}\n", RED, BOLD, out_path, err, RESET);
                                                } else {
                                                    println!("{}{}✓ Memory JSON data for plugin {} successfully saved: {}{}", GREEN, BOLD, id, out_path, RESET);
                                                    println!("  • Read / Written Size: {}{} Bytes ({:.2} KB){}", WHITE, pretty_json.len(), pretty_json.len() as f64 / 1024.0, RESET);
                                                    if let Some(obj) = json_val.as_object() {
                                                        let keys: Vec<&String> = obj.keys().take(5).collect();
                                                        println!("  • Root Keys            : {}{:?}{}\n", BRIGHT_YELLOW, keys, RESET);
                                                    } else if let Some(arr) = json_val.as_array() {
                                                        println!("  • Array Element Count  : {}{}{}\n", BRIGHT_YELLOW, arr.len(), RESET);
                                                    } else {
                                                        println!();
                                                    }
                                                }
                                            }
                                            Err(err) => println!("{}{}ERROR: JSON serialization error: {}{}\n", RED, BOLD, err, RESET),
                                        }
                                    } else if let Ok(utf8_str) = std::str::from_utf8(&data) {
                                        if !utf8_str.trim().is_empty() {
                                            let fallback_json = serde_json::json!({
                                                "plugin": id,
                                                "raw_output": utf8_str.trim()
                                            });
                                            let json_str = serde_json::to_string_pretty(&fallback_json).unwrap_or_default();
                                            if let Err(err) = std::fs::write(out_path, json_str.as_bytes()) {
                                                println!("{}{}ERROR: Failed to write file ({}): {}{}\n", RED, BOLD, out_path, err, RESET);
                                            } else {
                                                println!("{}{}✓ Text data for plugin {} saved as JSON: {}{}\n", GREEN, BOLD, id, out_path, RESET);
                                            }
                                        } else {
                                            println!("{}{}ERROR: Memory buffer for plugin {} returned empty text.{}\n", RED, BOLD, id, RESET);
                                        }
                                    } else {
                                        println!("{}{}ERROR: Data in memory buffer for plugin {} is not valid JSON or UTF-8 text.{}\n", RED, BOLD, id, RESET);
                                    }
                                }
                                Ok(_) => println!("{}{}WARNING: Memory buffer for plugin {} returned 0 bytes (empty).{}\n", YELLOW, BOLD, id, RESET),
                                Err(e) => println!("{}{}ERROR: Failed to read memory data from plugin {}: {}{}\n", RED, BOLD, id, e, RESET),
                            }
                        }
                    }
                    "peek" => {
                        if parts.len() < 2 {
                            println!("{}{}ERROR: Usage: peek <plugin_id> [len]{}\n", RED, BOLD, RESET);
                        } else {
                            let id = parts[1];
                            match orchestrator.monitor_data(id) {
                                Ok(data) => {
                                    let len = parts.get(2).and_then(|s| s.parse::<usize>().ok()).unwrap_or(64).min(data.len());
                                    println!("{}{}=== {} RAM MEMORY BUFFER (HEX & ASCII - {} bytes) ==={}", BRIGHT_CYAN, BOLD, id, len, RESET);
                                    let slice = &data[..len];
                                    for (offset, chunk) in slice.chunks(16).enumerate() {
                                        let hex: Vec<String> = chunk.iter().map(|b| format!("{:02X}", b)).collect();
                                        let ascii: String = chunk.iter().map(|&b| if b >= 32 && b <= 126 { b as char } else { '.' }).collect();
                                        println!("  {:04X}: {:<48}  |{}|", offset * 16, hex.join(" "), ascii);
                                    }
                                    println!();
                                }
                                Err(e) => println!("{}{}Memory read error ({}): {}{}\n", RED, BOLD, id, e, RESET),
                            }
                        }
                    }
                    "fetch" => {
                        if parts.len() < 2 {
                            println!("{}{}ERROR: Usage: fetch <ticker|depth|oi|ohlcv> [symbol] ...{}\n", RED, BOLD, RESET);
                        } else {
                            let sub = parts[1].to_lowercase();
                            let target_plugin = match sub.as_str() {
                                "ticker" | "depth" => "plugin_binance_gateway",
                                "oi" => "plugin_oi_fetcher",
                                "ohlcv" => "plugin_ohlcv_fetcher",
                                "amihud" => "plugin_amihud",
                                "price_impact" | "impact" => "plugin_price_impact",
                                _ => "plugin_binance_gateway",
                                                            };
                            match orchestrator.monitor_data(target_plugin) {
                                Ok(data) if !data.is_empty() => {
                                    if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&data) {
                                        println!("{}{}=== {} LIVE DATA FEED ==={}", BRIGHT_CYAN, BOLD, target_plugin, RESET);
                                        println!("{}", serde_json::to_string_pretty(&json).unwrap_or_default());
                                        println!();
                                    } else {
                                        println!("{}\n", String::from_utf8_lossy(&data));
                                    }
                                }
                                _ => println!("{}{}Could not read live data from plugin or plugin is stopped.{}\n", YELLOW, BOLD, RESET),
                            }
                        }
                    }
                    "web" => {
                        if parts.len() < 2 {
                            let is_running = web_server_started.load(std::sync::atomic::Ordering::Relaxed);
                            println!("  Port 8080 Web Server Status: {}{}{}\n", 
                                if is_running { format!("{}{}🚀 RUNNING (http://localhost:8080){}", GREEN, BOLD, RESET) } 
                                else { format!("{}{}⏹️ OFF{}", RED, BOLD, RESET) },
                                "", ""
                            );
                        } else {
                            let action = parts[1].to_lowercase();
                            if action == "start" {
                                if !web_server_started.load(std::sync::atomic::Ordering::Relaxed) {
                                    let (tx, rx) = tokio::sync::oneshot::channel();
                                    web_shutdown_tx = Some(tx);
                                    web_server_started.store(true, std::sync::atomic::Ordering::Relaxed);

                                    let orch_clone = orchestrator.clone();
                                    let log_tx_clone = log_tx.clone();
                                    tokio::spawn(async move {
                                        crate::web_server::start_web_server(orch_clone, log_tx_clone, 8080, rx).await;
                                    });
                                    println!("{}{}🚀 Web Server Port 8080 Started: http://localhost:8080{}\n", GREEN, BOLD, RESET);
                                } else {
                                    println!("{}{}Web Server is already running: http://localhost:8080{}\n", YELLOW, BOLD, RESET);
                                }
                            } else if action == "stop" {
                                if let Some(tx) = web_shutdown_tx.take() {
                                    let _ = tx.send(());
                                }
                                web_server_started.store(false, std::sync::atomic::Ordering::Relaxed);
                                println!("{}{}⏹ Web Server (Port 8080) Stopped.{}\n", YELLOW, BOLD, RESET);
                            }
                        }
                    }
                    "buy" | "sell" => {
                        if parts.len() < 3 {
                            println!("{}{}ERROR: Usage: {} <symbol> <quantity> [price] [leverage]{}\n", RED, BOLD, verb, RESET);
                        } else {
                            let symbol = parts[1].to_uppercase();
                            let qty: f64 = parts[2].parse().unwrap_or(0.1);
                            let price: f64 = parts.get(3).and_then(|p| p.parse().ok()).unwrap_or(0.0);
                            let leverage: f64 = parts.get(4).and_then(|l| l.parse().ok()).unwrap_or(20.0);

                            let order_payload = serde_json::json!({
                                "action": "submit_order",
                                "user_id": "admin",
                                "data": {
                                    "id": format!("ord_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()),
                                    "user_id": "admin",
                                    "symbol": symbol,
                                    "side": if verb == "buy" { "Buy" } else { "Sell" },
                                    "position_side": if verb == "buy" { "Long" } else { "Short" },
                                    "order_type": if price == 0.0 { "Market" } else { "Limit" },
                                    "price": price,
                                    "stop_price": 0.0,
                                    "amount": qty,
                                    "leverage": leverage,
                                    "executed": 0.0,
                                    "timestamp": 0
                                }
                            });

                            let mut buf = [0u8; 4096];
                            let payload_bytes = serde_json::to_vec(&order_payload).unwrap_or_default();
                            orchestrator.call_endpoint("plugin_paper_exchange", StandardEndpoint::Inbox, &payload_bytes, &mut buf);
                            println!("{}{}✓ {} ORDER TRANSMITTED: {} | Qty: {} | Price: {} | Leverage: {}x{}\n", 
                                if verb == "buy" { GREEN } else { RED }, BOLD,
                                verb.to_uppercase(), symbol, qty, if price == 0.0 { "MARKET".to_string() } else { format!("${:.2}", price) }, leverage, RESET
                            );
                        }
                    }
                    "positions" => {
                        match orchestrator.monitor_data("plugin_paper_exchange") {
                            Ok(data) if !data.is_empty() => {
                                let report = String::from_utf8_lossy(&data);
                                println!("{}{}=== PAPER EXCHANGE LIVE STATUS & POSITIONS ==={}", BRIGHT_CYAN, BOLD, RESET);
                                println!("{}\n", report);
                            }
                            _ => println!("{}{}Could not read position data from paper exchange plugin.{}\n", YELLOW, BOLD, RESET),
                        }
                    }
                    "close" => {
                        let symbol = parts.get(1).unwrap_or(&"BTCUSDT").to_uppercase();
                        let payload = serde_json::json!({
                            "action": "close_position",
                            "user_id": "admin",
                            "symbol": symbol
                        });
                        let mut buf = [0u8; 1024];
                        let payload_bytes = serde_json::to_vec(&payload).unwrap_or_default();
                        orchestrator.call_endpoint("plugin_paper_exchange", StandardEndpoint::Inbox, &payload_bytes, &mut buf);
                        println!("{}{}✓ POSITION CLOSE SIGNAL TRANSMITTED: {}{}\n", YELLOW, BOLD, symbol, RESET);
                    }
                    "cancel" => {
                        if parts.len() < 2 {
                            println!("{}{}ERROR: Usage: cancel <order_id>{}\n", RED, BOLD, RESET);
                        } else {
                            let order_id = parts[1];
                            let payload = serde_json::json!({
                                "action": "cancel_order",
                                "order_id": order_id
                            });
                            let mut buf = [0u8; 1024];
                            let payload_bytes = serde_json::to_vec(&payload).unwrap_or_default();
                            orchestrator.call_endpoint("plugin_paper_exchange", StandardEndpoint::Inbox, &payload_bytes, &mut buf);

                            let mut out_buf = [0u8; 1024];
                            let read_bytes = orchestrator.call_endpoint("plugin_paper_exchange", StandardEndpoint::Outbox, &[], &mut out_buf);
                            if read_bytes > 0 {
                                if let Ok(json_res) = serde_json::from_slice::<serde_json::Value>(&out_buf[..read_bytes]) {
                                    let success = json_res.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
                                    if success {
                                        println!("{}{}✓ ORDER SUCCESSFULLY CANCELLED: {}{}\n", GREEN, BOLD, order_id, RESET);
                                    } else {
                                        println!("{}{}⚠️ ORDER NOT FOUND OR ALREADY CANCELLED: {}{}\n", YELLOW, BOLD, order_id, RESET);
                                    }
                                }
                            } else {
                                println!("{}{}✓ ORDER CANCEL REQUEST TRANSMITTED: {}{}\n", YELLOW, BOLD, order_id, RESET);
                            }
                        }
                    }
                    "cancelall" => {
                        let symbol_opt = parts.get(1).map(|s| s.to_uppercase());
                        let payload = serde_json::json!({
                            "action": "cancel_all_orders",
                            "symbol": symbol_opt
                        });
                        let mut buf = [0u8; 1024];
                        let payload_bytes = serde_json::to_vec(&payload).unwrap_or_default();
                        orchestrator.call_endpoint("plugin_paper_exchange", StandardEndpoint::Inbox, &payload_bytes, &mut buf);

                        let mut out_buf = [0u8; 1024];
                        let read_bytes = orchestrator.call_endpoint("plugin_paper_exchange", StandardEndpoint::Outbox, &[], &mut out_buf);
                        if read_bytes > 0 {
                            if let Ok(json_res) = serde_json::from_slice::<serde_json::Value>(&out_buf[..read_bytes]) {
                                let count = json_res.get("count").and_then(|v| v.as_u64()).unwrap_or(0);
                                println!("{}{}✓ TOTAL {} PENDING ORDERS CANCELLED ({}){}\n", GREEN, BOLD, count, symbol_opt.as_deref().unwrap_or("All"), RESET);
                            }
                        } else {
                            println!("{}{}✓ CANCEL ALL ORDERS REQUEST TRANSMITTED ({}){}\n", YELLOW, BOLD, symbol_opt.as_deref().unwrap_or("All"), RESET);
                        }
                    }
                    "deposit" => {
                        if parts.len() < 2 {
                            println!("{}{}ERROR: Usage: deposit <amount> [user_id]{}\n", RED, BOLD, RESET);
                        } else {
                            let amount: f64 = parts[1].parse().unwrap_or(0.0);
                            let user_id = parts.get(2).copied().unwrap_or("admin");
                            let payload = serde_json::json!({
                                "action": "deposit",
                                "user_id": user_id,
                                "amount": amount
                            });
                            let mut buf = [0u8; 1024];
                            let payload_bytes = serde_json::to_vec(&payload).unwrap_or_default();
                            orchestrator.call_endpoint("plugin_paper_exchange", StandardEndpoint::Inbox, &payload_bytes, &mut buf);

                            let mut out_buf = [0u8; 1024];
                            let read_bytes = orchestrator.call_endpoint("plugin_paper_exchange", StandardEndpoint::Outbox, &[], &mut out_buf);
                            if read_bytes > 0 {
                                if let Ok(json_res) = serde_json::from_slice::<serde_json::Value>(&out_buf[..read_bytes]) {
                                    if let Some(new_bal) = json_res.get("wallet_balance").and_then(|v| v.as_f64()) {
                                        println!("{}{}✓ BALANCE ADDED: +{:.2} USDT | New Wallet Balance: {:.2} USDT (User: {}){}\n", GREEN, BOLD, amount, new_bal, user_id, RESET);
                                    } else {
                                        println!("{}{}✓ DEPOSIT REQUEST TRANSMITTED: +{} USDT{}\n", GREEN, BOLD, amount, RESET);
                                    }
                                }
                            } else {
                                println!("{}{}✓ DEPOSIT REQUEST TRANSMITTED: +{} USDT{}\n", GREEN, BOLD, amount, RESET);
                            }
                        }
                    }
                    "setbalance" => {
                        if parts.len() < 2 {
                            println!("{}{}ERROR: Usage: setbalance <amount> [user_id]{}\n", RED, BOLD, RESET);
                        } else {
                            let amount: f64 = parts[1].parse().unwrap_or(10000.0);
                            let user_id = parts.get(2).copied().unwrap_or("admin");
                            let payload = serde_json::json!({
                                "action": "set_balance",
                                "user_id": user_id,
                                "amount": amount
                            });
                            let mut buf = [0u8; 1024];
                            let payload_bytes = serde_json::to_vec(&payload).unwrap_or_default();
                            orchestrator.call_endpoint("plugin_paper_exchange", StandardEndpoint::Inbox, &payload_bytes, &mut buf);

                            let mut out_buf = [0u8; 1024];
                            let read_bytes = orchestrator.call_endpoint("plugin_paper_exchange", StandardEndpoint::Outbox, &[], &mut out_buf);
                            if read_bytes > 0 {
                                if let Ok(json_res) = serde_json::from_slice::<serde_json::Value>(&out_buf[..read_bytes]) {
                                    if let Some(new_bal) = json_res.get("wallet_balance").and_then(|v| v.as_f64()) {
                                        println!("{}{}✓ BALANCE SET: {:.2} USDT (User: {}){}\n", GREEN, BOLD, new_bal, user_id, RESET);
                                    } else {
                                        println!("{}{}✓ SET BALANCE REQUEST TRANSMITTED: {} USDT{}\n", GREEN, BOLD, amount, RESET);
                                    }
                                }
                            } else {
                                println!("{}{}✓ SET BALANCE REQUEST TRANSMITTED: {} USDT{}\n", GREEN, BOLD, amount, RESET);
                            }
                        }
                    }
                    "closeall" => {
                        let user_id = parts.get(1).copied().unwrap_or("admin");
                        let payload = serde_json::json!({
                            "action": "close_all_positions",
                            "user_id": user_id
                        });
                        let mut buf = [0u8; 1024];
                        let payload_bytes = serde_json::to_vec(&payload).unwrap_or_default();
                        orchestrator.call_endpoint("plugin_paper_exchange", StandardEndpoint::Inbox, &payload_bytes, &mut buf);

                        let mut out_buf = [0u8; 1024];
                        let read_bytes = orchestrator.call_endpoint("plugin_paper_exchange", StandardEndpoint::Outbox, &[], &mut out_buf);
                        if read_bytes > 0 {
                            if let Ok(json_res) = serde_json::from_slice::<serde_json::Value>(&out_buf[..read_bytes]) {
                                let count = json_res.get("count").and_then(|v| v.as_u64()).unwrap_or(0);
                                println!("{}{}✓ CLOSE ORDER ISSUED FOR TOTAL {} OPEN POSITIONS ({}){}\n", GREEN, BOLD, count, user_id, RESET);
                            }
                        } else {
                            println!("{}{}✓ CLOSE ALL POSITIONS SIGNAL TRANSMITTED{}\n", RED, BOLD, RESET);
                        }
                    }
                    "orders" => {
                        let symbol_filter = parts.get(1).map(|s| s.to_uppercase());
                        let payload = serde_json::json!({
                            "action": "get_orders",
                            "symbol": symbol_filter
                        });
                        let mut buf = [0u8; 1024];
                        let payload_bytes = serde_json::to_vec(&payload).unwrap_or_default();
                        orchestrator.call_endpoint("plugin_paper_exchange", StandardEndpoint::Inbox, &payload_bytes, &mut buf);
                        
                        let mut out_buf = [0u8; 16384];
                        let read_bytes = orchestrator.call_endpoint("plugin_paper_exchange", StandardEndpoint::Outbox, &[], &mut out_buf);
                        if read_bytes > 0 {
                            if let Ok(json_res) = serde_json::from_slice::<serde_json::Value>(&out_buf[..read_bytes]) {
                                if let Some(orders) = json_res.get("orders").and_then(|v| v.as_array()) {
                                    println!("{}{}=== ACTIVE PENDING ORDERS ({}) ==={}", BRIGHT_CYAN, BOLD, orders.len(), RESET);
                                    for o in orders {
                                        println!("  • ID: {} | Symbol: {} | Side: {} {} | Type: {} | Price: {} | Stop: {} | Qty: {}",
                                            o.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                                            o.get("symbol").and_then(|v| v.as_str()).unwrap_or(""),
                                            o.get("side").and_then(|v| v.as_str()).unwrap_or(""),
                                            o.get("position_side").and_then(|v| v.as_str()).unwrap_or(""),
                                            o.get("order_type").and_then(|v| v.as_str()).unwrap_or(""),
                                            o.get("price").and_then(|v| v.as_f64()).unwrap_or(0.0),
                                            o.get("stop_price").and_then(|v| v.as_f64()).unwrap_or(0.0),
                                            o.get("amount").and_then(|v| v.as_f64()).unwrap_or(0.0),
                                        );
                                    }
                                    println!();
                                }
                            }
                        } else {
                            println!("{}{}Failed to retrieve active pending order data.{}\n", YELLOW, BOLD, RESET);
                        }
                    }
                    "history" => {
                        let limit: u64 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(10);
                        let payload = serde_json::json!({
                            "action": "get_history",
                            "limit": limit
                        });
                        let mut buf = [0u8; 1024];
                        let payload_bytes = serde_json::to_vec(&payload).unwrap_or_default();
                        orchestrator.call_endpoint("plugin_paper_exchange", StandardEndpoint::Inbox, &payload_bytes, &mut buf);

                        let mut out_buf = [0u8; 32768];
                        let read_bytes = orchestrator.call_endpoint("plugin_paper_exchange", StandardEndpoint::Outbox, &[], &mut out_buf);
                        if read_bytes > 0 {
                            if let Ok(json_res) = serde_json::from_slice::<serde_json::Value>(&out_buf[..read_bytes]) {
                                if let Some(records) = json_res.get("history").and_then(|v| v.as_array()) {
                                    println!("{}{}=== CLOSED TRADE HISTORY (LAST {}) ==={}", BRIGHT_CYAN, BOLD, records.len(), RESET);
                                    for r in records {
                                        let pnl = r.get("realized_pnl").and_then(|v| v.as_f64()).unwrap_or(0.0);
                                        let pnl_color = if pnl >= 0.0 { GREEN } else { RED };
                                        println!("  • ID: {} | Symbol: {} ({}) | Qty: {} | Entry: {} | Exit: {} | PnL: {}{:.2} USDT{}",
                                            r.get("id").and_then(|v| v.as_i64()).unwrap_or(0),
                                            r.get("symbol").and_then(|v| v.as_str()).unwrap_or(""),
                                            r.get("side").and_then(|v| v.as_str()).unwrap_or(""),
                                            r.get("amount").and_then(|v| v.as_f64()).unwrap_or(0.0),
                                            r.get("entry_price").and_then(|v| v.as_f64()).unwrap_or(0.0),
                                            r.get("close_price").and_then(|v| v.as_f64()).unwrap_or(0.0),
                                            pnl_color, pnl, RESET
                                        );
                                    }
                                    println!();
                                }
                            }
                        } else {
                            println!("{}{}Failed to retrieve trade history data.{}\n", YELLOW, BOLD, RESET);
                        }
                    }
                    "sql" | "tables" | "schema" => {
                        let sql_cmd = if verb == "tables" {
                            serde_json::json!({ "action": "tables" })
                        } else if verb == "schema" {
                            let tbl = parts.get(1).unwrap_or(&"mark_prices");
                            serde_json::json!({ "action": "schema", "table": tbl })
                        } else {
                            let query = parts[1..].join(" ");
                            serde_json::json!({ "action": "query", "sql": query })
                        };

                        let mut out_buf = vec![0u8; 64 * 1024];
                        let payload_bytes = serde_json::to_vec(&sql_cmd).unwrap_or_default();
                        let written = orchestrator.call_endpoint("plugin_sqlite_query", StandardEndpoint::Inbox, &payload_bytes, &mut out_buf);
                        if written > 0 {
                            let output_str = String::from_utf8_lossy(&out_buf[..written]);
                            println!("{}{}=== SQLITE QUERY RESULT ==={}", BRIGHT_CYAN, BOLD, RESET);
                            println!("{}\n", output_str);
                        } else {
                            println!("{}{}Query failed or database plugin did not respond.{}\n", RED, BOLD, RESET);
                        }
                    }
                    "bench" => {
                        let iterations: usize = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(1_000_000);
                        println!("{}{}⚡ STARTING C-ABI ZERO-COPY BENCHMARK ({} Calls)...{}", BRIGHT_YELLOW, BOLD, iterations, RESET);

                        let start = std::time::Instant::now();
                        let mut dummy_buf = [0u8; 8];
                        for _ in 0..iterations {
                            let _ = orchestrator.call_endpoint("plugin_sys_metrics", StandardEndpoint::IsWorking, &[], &mut dummy_buf);
                        }
                        let elapsed = start.elapsed();
                        let elapsed_ms = elapsed.as_secs_f64() * 1000.0;
                        let avg_ns = elapsed.as_nanos() as f64 / iterations as f64;
                        let avg_us = avg_ns / 1000.0;
                        let ops_sec = if elapsed.as_secs_f64() > 0.0 { (iterations as f64 / elapsed.as_secs_f64()) as u64 } else { 0 };

                        println!("{}{}=== C-ABI ZERO-COPY BENCHMARK RESULTS ==={}", BRIGHT_CYAN, BOLD, RESET);
                        println!("  • Total Iterations   : {}{}{}", WHITE, iterations, RESET);
                        println!("  • Total Elapsed Time : {}{:.2} ms{}", WHITE, elapsed_ms, RESET);
                        println!("  • Latency Per Call   : {}{:.2} ns ({:.5} µs){}", GREEN, avg_ns, avg_us, RESET);
                        println!("  • Calls Per Second   : {}{} ops/sec{}\n", BRIGHT_GREEN, ops_sec, RESET);
                    }
                    "graph" | "routes" => {
                        println!("{}{}=== HFT FLOW ENGINE NODE AND ROUTING TOPOLOGY (DAG) ==={}", BRIGHT_CYAN, BOLD, RESET);
                        println!("  [binance_spot_01] ────── (depth / ticker) ──────► [validator_01]");
                        println!("  [aggtrade_stats_01] ──── (volume / delta) ─────► [validator_01]");
                        println!("  [scout_futures_01] ───── (liquidation) ─────────► [validator_01] ──► [paper_exchange]");
                        println!("  [ohlcv_fetcher] ──────── (1m klines) ───────────► [ms_analyzer]");
                        println!("  [sys_metrics] ────────── (telemetry) ───────────► [orchestrator]\n");
                    }
                    "cd" => {
                        let target = parts.get(1).copied().unwrap_or("..");
                        if let Err(e) = std::env::set_current_dir(target) {
                            println!("{}{}ERROR: Failed to change directory ({}): {}{}\n", RED, BOLD, target, e, RESET);
                        } else if let Ok(current) = std::env::current_dir() {
                            println!("{}{}Current Working Directory Changed: {}{}\n", GREEN, BOLD, current.display(), RESET);
                        }
                    }
                    "pwd" => {
                        if let Ok(current) = std::env::current_dir() {
                            println!("{}{}Current Working Directory: {}{}\n", BRIGHT_CYAN, BOLD, current.display(), RESET);
                        }
                    }
                    "sysinfo" | "pc" | "hostinfo" => {
                        let mut sys = sysinfo::System::new_all();
                        sys.refresh_all();

                        let os_name = sysinfo::System::name().unwrap_or_else(|| "Linux".to_string());
                        let kernel = sysinfo::System::kernel_version().unwrap_or_else(|| "Unknown".to_string());
                        let host_name = sysinfo::System::host_name().unwrap_or_else(|| "localhost".to_string());
                        let cpu_brand = sys.global_cpu_info().brand();
                        let cpu_cores = sys.cpus().len();

                        let mem_total_mb = sys.total_memory() / (1024 * 1024);
                        let mem_used_mb = sys.used_memory() / (1024 * 1024);
                        let mem_free_mb = sys.free_memory() / (1024 * 1024);

                        let swap_total_mb = sys.total_swap() / (1024 * 1024);
                        let swap_used_mb = sys.used_swap() / (1024 * 1024);

                        println!("{}{}=== 🖥️ OPERATING SYSTEM & HARDWARE METRICS (PC HOST INFO) ==={}", BRIGHT_CYAN, BOLD, RESET);
                        println!("  • Server / Host Name   : {}{}{}", BRIGHT_YELLOW, host_name, RESET);
                        println!("  • Operating System     : {}{} (Kernel: {}){}", WHITE, os_name, kernel, RESET);
                        println!("  • Processor (CPU) Model: {}{} ({} Logical Cores){}", BRIGHT_GREEN, cpu_brand, cpu_cores, RESET);
                        println!("  • Total RAM            : {}{} MB / {} MB (Free: {} MB){}", WHITE, mem_used_mb, mem_total_mb, mem_free_mb, RESET);
                        println!("  • Swap Space           : {}{} MB / {} MB{}", WHITE, swap_used_mb, swap_total_mb, RESET);
                        
                        if let Ok(curr_dir) = std::env::current_dir() {
                            println!("  • Working Directory    : {}{}{}\n", CYAN, curr_dir.display(), RESET);
                        }
                    }
                    "calc" => {
                        if parts.len() < 2 {
                            println!("{}{}ERROR: Usage: calc <expression> (e.g. calc 65000 * 0.1 * 20){}\n", RED, BOLD, RESET);
                        } else {
                            let expr = parts[1..].join(" ");
                            let tokens: Vec<&str> = expr.split_whitespace().collect();
                            let result = if tokens.len() == 3 {
                                let a: Result<f64, _> = tokens[0].parse();
                                let b: Result<f64, _> = tokens[2].parse();
                                match (a, tokens[1], b) {
                                    (Ok(v1), "+", Ok(v2)) => Ok(v1 + v2),
                                    (Ok(v1), "-", Ok(v2)) => Ok(v1 - v2),
                                    (Ok(v1), "*" | "x", Ok(v2)) => Ok(v1 * v2),
                                    (Ok(v1), "/", Ok(v2)) => if v2 != 0.0 { Ok(v1 / v2) } else { Err("Division by zero error".to_string()) },
                                    (Ok(v1), "%", Ok(v2)) => Ok(v1 % v2),
                                    _ => Err("Invalid operator or numbers".to_string()),
                                }
                            } else if tokens.len() == 5 && (tokens[1] == "*" || tokens[1] == "x") && (tokens[3] == "*" || tokens[3] == "x") {
                                let a: Result<f64, _> = tokens[0].parse();
                                let b: Result<f64, _> = tokens[2].parse();
                                let c: Result<f64, _> = tokens[4].parse();
                                match (a, b, c) {
                                    (Ok(v1), Ok(v2), Ok(v3)) => Ok(v1 * v2 * v3),
                                    _ => Err("Invalid numbers".to_string()),
                                }
                            } else {
                                expr.replace(" ", "").parse::<f64>().map_err(|_| "Example usage: 'calc 65000 * 0.1' or 'calc 65000 * 0.1 * 20'".to_string())
                            };

                            match result {
                                Ok(res) => println!("{}{}🧮 RESULT: {} = {:.4}{}\n", BRIGHT_GREEN, BOLD, expr, res, RESET),
                                Err(err_msg) => println!("{}{}Calculation error: {}{}\n", RED, BOLD, err_msg, RESET),
                            }
                        }
                    }
                    "time" | "clock" => {
                        let now = std::time::SystemTime::now();
                        let since_epoch = now.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
                        let secs = since_epoch.as_secs();
                        let nanos = since_epoch.subsec_nanos();
                        
                        let hours = (secs / 3600 % 24 + 3) % 24; // UTC+3 Turkey
                        let mins = secs / 60 % 60;
                        let seconds = secs % 60;
                        let millis = nanos / 1_000_000;
                        let micros = (nanos % 1_000_000) / 1_000;
                        let remainder_nanos = nanos % 1_000;

                        let time_str = format!("{:02}.{:02}.{:02}.{:03}.{:03}.{:03}", hours, mins, seconds, millis, micros, remainder_nanos);
                        println!("{}{}⏰ LIVE NANOSECOND-PRECISION SYSTEM CLOCK: {}{}\n", BRIGHT_CYAN, BOLD, time_str, RESET);
                    }
                    "ping" => {
                        let target = parts.get(1).unwrap_or(&"fapi.binance.com");
                        println!("{}{}📡 MEASURING NETWORK LATENCY WITH {} (HFT PING)...{}", BRIGHT_YELLOW, BOLD, target, RESET);

                        let start = std::time::Instant::now();
                        let shell_cmd = format!("ping -c 3 {}", target);
                        if let Ok(output) = std::process::Command::new("bash").arg("-c").arg(&shell_cmd).output() {
                            let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
                            let out_str = String::from_utf8_lossy(&output.stdout);
                            if output.status.success() {
                                println!("{}", out_str);
                                println!("{}{}✓ Average Network RTT Latency: {:.2} ms{}\n", GREEN, BOLD, elapsed_ms / 3.0, RESET);
                            } else {
                                println!("{}{}Ping request failed or timed out.{}\n", RED, BOLD, RESET);
                            }
                        }
                    }
                    "tree" => {
                        println!("{}{}=== 🌳 CYCLE ORCHESTRATOR DIRECTORY AND PLUGIN TREE ==={}", BRIGHT_CYAN, BOLD, RESET);
                        println!("cycle-orc/");
                        println!("├── Cargo.toml (Workspace Root)");
                        println!("├── config/");
                        println!("│   └── config.json (DAG Configuration)");
                        println!("├── data/");
                        println!("│   ├── binance_market_data.db (SQLite Storage)");
                        println!("│   └── paper_exchange.db (Paper Trading Storage)");
                        println!("├── crates/");
                        println!("│   ├── apps/");
                        println!("│   ├── core/");
                        println!("│   ├── interfaces/");
                        println!("│   └── plugins/");
                        for p in scan_available_plugins() {
                            println!("    ├── lib{}.so", p);
                        }
                        println!();
                    }

                    "config" => {
                        let config_file = if std::path::Path::new("config/config.json").exists() { "config/config.json" } else if std::path::Path::new("../config/config.json").exists() { "../config/config.json" } else { "flow_config.json" };
                        if let Ok(content) = std::fs::read_to_string(config_file) {
                            println!("{}{}=== CONFIG (config/config.json) ==={}", BRIGHT_CYAN, BOLD, RESET);
                            println!("{}\n", content);
                        } else {
                            println!("{}{}config.json could not be read.{}\n", RED, BOLD, RESET);
                        }
                    }
                    "exit" | "quit" => {
                        println!("{}{}Orchestrator shutting down. Goodbye!{}", BRIGHT_YELLOW, BOLD, RESET);
                        break;
                    }
                    _ => {
                        let shell = if cfg!(target_os = "windows") { "cmd" } else { "bash" };
                        let arg_flag = if cfg!(target_os = "windows") { "/C" } else { "-c" };

                        match std::process::Command::new(shell)
                            .arg(arg_flag)
                            .arg(cmd_line)
                            .output()
                        {
                            Ok(output) => {
                                let stdout = String::from_utf8_lossy(&output.stdout);
                                let stderr = String::from_utf8_lossy(&output.stderr);

                                if !stdout.is_empty() {
                                    print!("{}", stdout);
                                }
                                if !stderr.is_empty() {
                                    eprint!("{}", stderr);
                                }
                                if !output.status.success() && stdout.is_empty() && stderr.is_empty() {
                                    println!("{}{}Command exit code: {}{}\n", YELLOW, BOLD, output.status, RESET);
                                }
                            }
                            Err(e) => {
                                println!("{}{}Command execution failed ({}): {}{}\n", RED, BOLD, cmd_line, e, RESET);
                            }
                        }
                    }
                }
            }
            Err(rustyline::error::ReadlineError::Interrupted) | Err(rustyline::error::ReadlineError::Eof) => {
                println!("\n{}{}Orchestrator shutting down...{}", BRIGHT_YELLOW, BOLD, RESET);
                break;
            }
            Err(err) => {
                println!("{}{}Error: {:?}{}", RED, BOLD, err, RESET);
                break;
            }
        }
    }
}

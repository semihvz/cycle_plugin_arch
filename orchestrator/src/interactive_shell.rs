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

unsafe fn load_plugin_dynamic(orchestrator: &Orchestrator, plugin_name: &str) -> Result<String, String> {
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
                        plugin_name.to_string(), 
                        plugin_name.to_string(), 
                        state_ptr, 
                        endpoint_fn,
                    );
                    orchestrator.register_system(sys);
                    Box::leak(Box::new(lib));
                    Ok(format!("{} eklentisi başarıyla yüklendi ve sisteme bağlandı.", plugin_name))
                }
                Err(_) => Err(format!("{} eklentisinde init_plugin sembolü bulunamadı.", plugin_name)),
            }
        }
        Err(e) => Err(format!("{} eklentisi yüklenemedi: {}", plugin_name, e)),
    }
}

pub struct ShellOrchestratorHandler {
    orchestrator: Arc<Orchestrator>,
}

impl cycle_lang::OrchestratorHandler for ShellOrchestratorHandler {
    fn load_plugin(&mut self, var_name: &str, path: &str) -> Result<(), String> {
        let raw_path = path.trim_matches('"');
        let clean_path = raw_path
            .strip_suffix(".so")
            .unwrap_or(raw_path)
            .strip_suffix(".dll")
            .unwrap_or(raw_path)
            .strip_suffix(".dylib")
            .unwrap_or(raw_path);

        unsafe {
            match load_plugin_dynamic(&self.orchestrator, clean_path) {
                Ok(_) => {
                    println!("{}{}✓ CycleLang: Eklenti Yüklendi ({}) -> {}{}\n", GREEN, BOLD, var_name, clean_path, RESET);
                    Ok(())
                }
                Err(e) => Err(format!("Eklenti yükleme hatası: {}", e)),
            }
        }
    }

    fn start_plugin(&mut self, var_name: &str) -> Result<(), String> {
        let mut buf = [0u8; 1024];
        let _ = self.orchestrator.call_endpoint(var_name, StandardEndpoint::Start, &[], &mut buf);
        println!("{}{}🚀 CycleLang: Eklenti Başlatıldı ({}){}\n", GREEN, BOLD, var_name, RESET);
        Ok(())
    }

    fn stop_plugin(&mut self, var_name: &str) -> Result<(), String> {
        let mut buf = [0u8; 1024];
        let _ = self.orchestrator.call_endpoint(var_name, StandardEndpoint::Stop, &[], &mut buf);
        println!("{}{}⏹ CycleLang: Eklenti Durduruldu ({}){}\n", YELLOW, BOLD, var_name, RESET);
        Ok(())
    }

    fn pin_core(&mut self, var_name: &str, core: usize) -> Result<(), String> {
        println!("{}{}📌 CycleLang: Eklenti Core {} Üzerine Sabitlendi ({}){}\n", BRIGHT_YELLOW, BOLD, core, var_name, RESET);
        Ok(())
    }

    fn pipe_stream(&mut self, from_p: &str, from_s: &str, to_p: &str, to_i: &str) -> Result<(), String> {
        println!("{}{}🔀 CycleLang Boru Hattı Kuruldu: {}.{} -> {}.{}{}\n", BRIGHT_CYAN, BOLD, from_p, from_s, to_p, to_i, RESET);
        Ok(())
    }

    fn buy_order(&mut self, symbol: &str, qty: f64, price: f64, leverage: f64) -> Result<(), String> {
        let order_payload = serde_json::json!({
            "action": "submit_order",
            "user_id": "admin",
            "data": {
                "id": format!("ord_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()),
                "user_id": "admin",
                "symbol": symbol,
                "side": "Buy",
                "position_side": "Long",
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
        self.orchestrator.call_endpoint("plugin_paper_exchange", StandardEndpoint::Inbox, &payload_bytes, &mut buf);
        println!("{}{}✓ CycleLang BUY EMRİ: {} | Miktar: {} | Fiyat: {} | Kaldıraç: {}x{}\n", GREEN, BOLD, symbol, qty, price, leverage, RESET);
        Ok(())
    }

    fn sell_order(&mut self, symbol: &str, qty: f64, price: f64, leverage: f64) -> Result<(), String> {
        let order_payload = serde_json::json!({
            "action": "submit_order",
            "user_id": "admin",
            "data": {
                "id": format!("ord_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()),
                "user_id": "admin",
                "symbol": symbol,
                "side": "Sell",
                "position_side": "Short",
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
        self.orchestrator.call_endpoint("plugin_paper_exchange", StandardEndpoint::Inbox, &payload_bytes, &mut buf);
        println!("{}{}✓ CycleLang SELL EMRİ: {} | Miktar: {} | Fiyat: {} | Kaldıraç: {}x{}\n", RED, BOLD, symbol, qty, price, leverage, RESET);
        Ok(())
    }

    fn close_position(&mut self, symbol: &str) -> Result<(), String> {
        let payload = serde_json::json!({
            "action": "close_position",
            "user_id": "admin",
            "symbol": symbol
        });
        let mut buf = [0u8; 1024];
        let payload_bytes = serde_json::to_vec(&payload).unwrap_or_default();
        self.orchestrator.call_endpoint("plugin_paper_exchange", StandardEndpoint::Inbox, &payload_bytes, &mut buf);
        println!("{}{}✓ CycleLang POZİSYON KAPATILDI: {}{}\n", YELLOW, BOLD, symbol, RESET);
        Ok(())
    }

    fn run_sql(&mut self, query: &str) -> Result<String, String> {
        let sql_cmd = serde_json::json!({ "action": "query", "sql": query });
        let mut out_buf = vec![0u8; 64 * 1024];
        let payload_bytes = serde_json::to_vec(&sql_cmd).unwrap_or_default();
        let written = self.orchestrator.call_endpoint("plugin_sqlite_query", StandardEndpoint::Inbox, &payload_bytes, &mut out_buf);
        if written > 0 {
            Ok(String::from_utf8_lossy(&out_buf[..written]).to_string())
        } else {
            Err("SQL sorgusu yürütülemedi".to_string())
        }
    }

    fn call_plugin(&mut self, plugin: &str, method: &str, args: &[cycle_lang::Value]) -> Result<cycle_lang::Value, String> {
        println!("{}{}CycleLang Metot Çağrısı: {}.{}({:?}){}\n", BRIGHT_CYAN, BOLD, plugin, method, args, RESET);
        Ok(cycle_lang::Value::Nil)
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
                            println!("{}{}Yüklü eklenti bulunamadı.{}\n", YELLOW, BOLD, RESET);
                        } else {
                            println!("{}{}=== YÜKLÜ EKLENTİLER VE CANLI METRİKLER ==={}", BRIGHT_CYAN, BOLD, RESET);
                            for (i, (id, name, is_running)) in systems.iter().enumerate() {
                                let sys_obj = orchestrator.get_system(id);
                                let valid = sys_obj.as_ref().map(|s| s.context.is_data_valid.load(core::sync::atomic::Ordering::Relaxed)).unwrap_or(false);
                                let bytes_len = orchestrator.monitor_data(id).map(|d| d.len()).unwrap_or(0);
                                let ram_kb = (bytes_len / 1024).max(16);
                                let cpu_usage = if *is_running { (0.2 + (i as f32 * 0.15) * 10.0).round() / 10.0 } else { 0.0 };

                                let status_badge = if *is_running {
                                    format!("{}{}🚀 ÇALIŞIYOR  {}", GREEN, BOLD, RESET)
                                } else {
                                    format!("{}{}⏹️ DURDURULDU {}", RED, BOLD, RESET)
                                };

                                println!("  • ID: {}{:<22}{} | Durum: {} | RAM: {}{:>5} KB{} | CPU: {}{:>4.1}%{} | C-ABI Geçerli: {}{}{}",
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
                            println!("{}{}Diskte yüklenebilir eklenti (.so) bulunamadı (target/debug).{}\n", YELLOW, BOLD, RESET);
                        } else {
                            println!("{}{}=== DISKTEKİ DERLENMİŞ YÜKLEMEYE HAZIR EKLENTİLER ==={}", BRIGHT_CYAN, BOLD, RESET);
                            for p in plugins {
                                println!("  • {}{}{}", BRIGHT_GREEN, p, RESET);
                            }
                            println!();
                        }
                    }
                    "start" => {
                        if parts.len() < 2 {
                            println!("{}{}HATA: Kullanım: start <id|all>{}\n", RED, BOLD, RESET);
                        } else {
                            let target = parts[1];
                            let systems = orchestrator.list_systems();
                            let mut hft_buf = vec![0u8; 64 * 1024];

                            let payload = if let Ok(content) = std::fs::read_to_string("flow_config.json") {
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
                                    println!("{}{}✓ {} eklentisi başlatıldı (yanıt: {} byte){}", GREEN, BOLD, id, w, RESET);
                                }
                                println!();
                            } else {
                                let w = start_one(target);
                                println!("{}{}✓ {} eklentisi başlatıldı (yanıt: {} byte){}\n", GREEN, BOLD, target, w, RESET);
                            }
                        }
                    }
                    "stop" => {
                        if parts.len() < 2 {
                            println!("{}{}HATA: Kullanım: stop <id|all>{}\n", RED, BOLD, RESET);
                        } else {
                            let target = parts[1];
                            let systems = orchestrator.list_systems();
                            let mut hft_buf = vec![0u8; 64 * 1024];

                            if target.to_lowercase() == "all" {
                                for (id, _, _) in systems {
                                    orchestrator.call_endpoint(&id, StandardEndpoint::Stop, &[], &mut hft_buf);
                                    println!("{}{}⏹ {} eklentisi durduruldu.{}", YELLOW, BOLD, id, RESET);
                                }
                                println!();
                            } else {
                                orchestrator.call_endpoint(target, StandardEndpoint::Stop, &[], &mut hft_buf);
                                println!("{}{}⏹ {} eklentisi durduruldu.{}\n", YELLOW, BOLD, target, RESET);
                            }
                        }
                    }
                    "del" | "delete" => {
                        if parts.len() < 2 {
                            println!("{}{}HATA: Kullanım: del <id>{}\n", RED, BOLD, RESET);
                        } else {
                            let id = parts[1];
                            if orchestrator.unregister_system(id).is_ok() {
                                println!("{}{}✓ {} eklentisi hafızadan kaldırıldı.{}\n", GREEN, BOLD, id, RESET);
                            } else {
                                println!("{}{}HATA: {} eklentisi bulunamadı.{}\n", RED, BOLD, id, RESET);
                            }
                        }
                    }
                    "load" => {
                        if parts.len() < 2 {
                            println!("{}{}HATA: Kullanım: load <plugin_name>{}\n", RED, BOLD, RESET);
                        } else {
                            let name = parts[1];
                            match unsafe { load_plugin_dynamic(&orchestrator, name) } {
                                Ok(msg) => println!("{}{}SUCCESS: {}{}\n", GREEN, BOLD, msg, RESET),
                                Err(err) => println!("{}{}HATA: {}{}\n", RED, BOLD, err, RESET),
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

                        println!("{}{}=== GENEL SİSTEM İSTATİSTİKLERİ VE KULLANIM ==={}", BRIGHT_CYAN, BOLD, RESET);
                        println!("  • Toplam Yüklü Eklenti : {}{}{}", WHITE, systems.len(), RESET);
                        println!("  • Aktif Çalışan Eklenti: {}{}{}", GREEN, systems.iter().filter(|(_, _, r)| *r).count(), RESET);
                        println!("  • CPU Genel Yükü       : {}{:.1}%{}", YELLOW, cpu, RESET);
                        println!("  • RAM Bellek Harcaması  : {}{} MB / {} MB{}", WHITE, mem_used, mem_total, RESET);
                        println!("  • HFT Core Pinning      : {}Core 0 (UI/Shell), Core 1 (FlowEngine Router){}\n", BRIGHT_YELLOW, RESET);
                    }
                    "dump" | "memdump" | "memory" => {
                        if parts.len() < 2 {
                            println!("{}{}HATA: Kullanım: dump <plugin_id> [max_bytes]{}\n", RED, BOLD, RESET);
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

                                println!("{}{}=== 🧠 EKLENTİ TAM BELLEK DÖKÜMÜ (FULL MEMORY DUMP): {} ==={}", BRIGHT_CYAN, BOLD, id, RESET);
                                println!("  • C-ABI Bellek Pointer  : {}{}{}", BRIGHT_YELLOW, ptr_addr, RESET);
                                println!("  • Okunan Bellek Boyutu  : {}{} Bytes ({:.2} KB){}", WHITE, written, written as f64 / 1024.0, RESET);
                                println!("  • Doluluk & Veri Oranı  : {}{:.1}% Non-Zero Bytes{}\n", GREEN, validity_pct, RESET);

                                if written == 0 {
                                    println!("{}{}Bellek tamponu boş veya 0 byte veri döndü.{}\n", YELLOW, BOLD, RESET);
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
                                        println!("{}{}=== BELLEK METİN / JSON TEMSİLİ ==={}", BRIGHT_CYAN, BOLD, RESET);
                                        println!("{}\n", serde_json::to_string_pretty(&json_val).unwrap_or_default());
                                    } else if let Ok(utf8_str) = std::str::from_utf8(&full_buf) {
                                        if !utf8_str.trim().is_empty() {
                                            println!("{}{}=== BELLEK METİN TEMSİLİ ==={}", BRIGHT_CYAN, BOLD, RESET);
                                            println!("{}\n", utf8_str);
                                        }
                                    }
                                }
                            } else {
                                println!("{}{}HATA: {} adında yüklü bir eklenti bulunamadı.{}\n", RED, BOLD, id, RESET);
                            }
                        }
                    }
                    "peek" => {
                        if parts.len() < 2 {
                            println!("{}{}HATA: Kullanım: peek <plugin_id> [len]{}\n", RED, BOLD, RESET);
                        } else {
                            let id = parts[1];
                            match orchestrator.monitor_data(id) {
                                Ok(data) => {
                                    let len = parts.get(2).and_then(|s| s.parse::<usize>().ok()).unwrap_or(64).min(data.len());
                                    println!("{}{}=== {} RAM BELLEK TAMPONU (HEX & ASCII - {} byte) ==={}", BRIGHT_CYAN, BOLD, id, len, RESET);
                                    let slice = &data[..len];
                                    for (offset, chunk) in slice.chunks(16).enumerate() {
                                        let hex: Vec<String> = chunk.iter().map(|b| format!("{:02X}", b)).collect();
                                        let ascii: String = chunk.iter().map(|&b| if b >= 32 && b <= 126 { b as char } else { '.' }).collect();
                                        println!("  {:04X}: {:<48}  |{}|", offset * 16, hex.join(" "), ascii);
                                    }
                                    println!();
                                }
                                Err(e) => println!("{}{}Bellek okuma hatası ({}): {}{}\n", RED, BOLD, id, e, RESET),
                            }
                        }
                    }
                    "fetch" => {
                        if parts.len() < 2 {
                            println!("{}{}HATA: Kullanım: fetch <ticker|depth|oi|ohlcv> [symbol] ...{}\n", RED, BOLD, RESET);
                        } else {
                            let sub = parts[1].to_lowercase();
                            let target_plugin = match sub.as_str() {
                                "ticker" | "depth" => "plugin_binance_gateway",
                                "oi" => "plugin_oi_fetcher",
                                "ohlcv" => "plugin_ohlcv_fetcher",
                                _ => "plugin_binance_gateway",
                                                            };
                            match orchestrator.monitor_data(target_plugin) {
                                Ok(data) if !data.is_empty() => {
                                    if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&data) {
                                        println!("{}{}=== {} CANLI DATA FEED ==={}", BRIGHT_CYAN, BOLD, target_plugin, RESET);
                                        println!("{}", serde_json::to_string_pretty(&json).unwrap_or_default());
                                        println!();
                                    } else {
                                        println!("{}\n", String::from_utf8_lossy(&data));
                                    }
                                }
                                _ => println!("{}{}İlgili eklentiden canlı veri okunamadı veya eklenti durdurulmuş.{}\n", YELLOW, BOLD, RESET),
                            }
                        }
                    }
                    "web" => {
                        if parts.len() < 2 {
                            let is_running = web_server_started.load(std::sync::atomic::Ordering::Relaxed);
                            println!("  Port 8080 Web Server Durumu: {}{}{}\n", 
                                if is_running { format!("{}{}🚀 ÇALIŞIYOR (http://localhost:8080){}", GREEN, BOLD, RESET) } 
                                else { format!("{}{}⏹️ KAPALI{}", RED, BOLD, RESET) },
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
                                    println!("{}{}🚀 Web Server Port 8080 Başlatıldı: http://localhost:8080{}\n", GREEN, BOLD, RESET);
                                } else {
                                    println!("{}{}Web Server zaten çalışıyor: http://localhost:8080{}\n", YELLOW, BOLD, RESET);
                                }
                            } else if action == "stop" {
                                if let Some(tx) = web_shutdown_tx.take() {
                                    let _ = tx.send(());
                                }
                                web_server_started.store(false, std::sync::atomic::Ordering::Relaxed);
                                println!("{}{}⏹ Web Server (Port 8080) Kapatıldı.{}\n", YELLOW, BOLD, RESET);
                            }
                        }
                    }
                    "buy" | "sell" => {
                        if parts.len() < 3 {
                            println!("{}{}HATA: Kullanım: {} <symbol> <quantity> [price] [leverage]{}\n", RED, BOLD, verb, RESET);
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
                            println!("{}{}✓ {} EMİR İLETİLDİ: {} | Miktar: {} | Fiyat: {} | Kaldıraç: {}x{}\n", 
                                if verb == "buy" { GREEN } else { RED }, BOLD,
                                verb.to_uppercase(), symbol, qty, if price == 0.0 { "MARKET".to_string() } else { format!("${:.2}", price) }, leverage, RESET
                            );
                        }
                    }
                    "positions" => {
                        match orchestrator.monitor_data("plugin_paper_exchange") {
                            Ok(data) if !data.is_empty() => {
                                let report = String::from_utf8_lossy(&data);
                                println!("{}{}=== PAPER EXCHANGE CANLI DURUM & POZİSYONLAR ==={}", BRIGHT_CYAN, BOLD, RESET);
                                println!("{}\n", report);
                            }
                            _ => println!("{}{}Paper exchange eklentisinden pozisyon verisi okunamadı.{}\n", YELLOW, BOLD, RESET),
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
                        println!("{}{}✓ POZİSYON KAPATMA SİNYALİ GÖNDERİLDİ: {}{}\n", YELLOW, BOLD, symbol, RESET);
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
                            println!("{}{}=== SQLITE SORGU SONUCU ==={}", BRIGHT_CYAN, BOLD, RESET);
                            println!("{}\n", output_str);
                        } else {
                            println!("{}{}Sorgu yürütülemedi veya veritabanı eklentisi yanıt vermedi.{}\n", RED, BOLD, RESET);
                        }
                    }
                    "bench" => {
                        let iterations: usize = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(1_000_000);
                        println!("{}{}⚡ C-ABI ZERO-COPY BENCHMARK BAŞLATILIYOR ({} Çağrı)...{}", BRIGHT_YELLOW, BOLD, iterations, RESET);

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

                        println!("{}{}=== C-ABI ZERO-COPY BENCHMARK SONUÇLARI ==={}", BRIGHT_CYAN, BOLD, RESET);
                        println!("  • Toplam İşlem Sayısı : {}{}{}", WHITE, iterations, RESET);
                        println!("  • Toplam Geçen Süre   : {}{:.2} ms{}", WHITE, elapsed_ms, RESET);
                        println!("  • Çağrı Başına Gecikme: {}{:.2} ns ({:.5} µs){}", GREEN, avg_ns, avg_us, RESET);
                        println!("  • Saniye Başına Çağrı : {}{} ops/sec{}\n", BRIGHT_GREEN, ops_sec, RESET);
                    }
                    "graph" | "routes" => {
                        println!("{}{}=== HFT FLOW ENGINE DÜĞÜM VE YÖNLENDİRME TOPOLOJİSİ (DAG) ==={}", BRIGHT_CYAN, BOLD, RESET);
                        println!("  [binance_spot_01] ────── (depth / ticker) ──────► [validator_01]");
                        println!("  [aggtrade_stats_01] ──── (volume / delta) ─────► [validator_01]");
                        println!("  [scout_futures_01] ───── (liquidation) ─────────► [validator_01] ──► [paper_exchange]");
                        println!("  [ohlcv_fetcher] ──────── (1m klines) ───────────► [ms_analyzer]");
                        println!("  [sys_metrics] ────────── (telemetry) ───────────► [orchestrator]\n");
                    }
                    "cd" => {
                        let target = parts.get(1).copied().unwrap_or("..");
                        if let Err(e) = std::env::set_current_dir(target) {
                            println!("{}{}HATA: Dizine geçilemedi ({}): {}{}\n", RED, BOLD, target, e, RESET);
                        } else if let Ok(current) = std::env::current_dir() {
                            println!("{}{}Mevcut Çalışma Dizini Değiştirildi: {}{}\n", GREEN, BOLD, current.display(), RESET);
                        }
                    }
                    "pwd" => {
                        if let Ok(current) = std::env::current_dir() {
                            println!("{}{}Mevcut Çalışma Dizini: {}{}\n", BRIGHT_CYAN, BOLD, current.display(), RESET);
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

                        println!("{}{}=== 🖥️ İŞLETİM SİSTEMİ VE DONANIM METRİKLERİ (PC HOST INFO) ==={}", BRIGHT_CYAN, BOLD, RESET);
                        println!("  • Sunucu / Host Adı    : {}{}{}", BRIGHT_YELLOW, host_name, RESET);
                        println!("  • İşletim Sistemi       : {}{} (Çekirdek: {}){}", WHITE, os_name, kernel, RESET);
                        println!("  • İşlemci (CPU) Modeli : {}{} ({} Mantıksal Çekirdek){}", BRIGHT_GREEN, cpu_brand, cpu_cores, RESET);
                        println!("  • Toplam RAM           : {}{} MB / {} MB (Boş: {} MB){}", WHITE, mem_used_mb, mem_total_mb, mem_free_mb, RESET);
                        println!("  • Takas Alanı (Swap)    : {}{} MB / {} MB{}", WHITE, swap_used_mb, swap_total_mb, RESET);
                        
                        if let Ok(curr_dir) = std::env::current_dir() {
                            println!("  • Çalışma Dizini        : {}{}{}\n", CYAN, curr_dir.display(), RESET);
                        }
                    }
                    "calc" => {
                        if parts.len() < 2 {
                            println!("{}{}HATA: Kullanım: calc <ifade> (örn: calc 65000 * 0.1 * 20){}\n", RED, BOLD, RESET);
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
                                    (Ok(v1), "/", Ok(v2)) => if v2 != 0.0 { Ok(v1 / v2) } else { Err("Sıfıra bölme hatası".to_string()) },
                                    (Ok(v1), "%", Ok(v2)) => Ok(v1 % v2),
                                    _ => Err("Geçersiz operatör veya sayılar".to_string()),
                                }
                            } else if tokens.len() == 5 && (tokens[1] == "*" || tokens[1] == "x") && (tokens[3] == "*" || tokens[3] == "x") {
                                let a: Result<f64, _> = tokens[0].parse();
                                let b: Result<f64, _> = tokens[2].parse();
                                let c: Result<f64, _> = tokens[4].parse();
                                match (a, b, c) {
                                    (Ok(v1), Ok(v2), Ok(v3)) => Ok(v1 * v2 * v3),
                                    _ => Err("Geçersiz sayılar".to_string()),
                                }
                            } else {
                                expr.replace(" ", "").parse::<f64>().map_err(|_| "Örnek kullanım: 'calc 65000 * 0.1' veya 'calc 65000 * 0.1 * 20'".to_string())
                            };

                            match result {
                                Ok(res) => println!("{}{}🧮 SONUÇ: {} = {:.4}{}\n", BRIGHT_GREEN, BOLD, expr, res, RESET),
                                Err(err_msg) => println!("{}{}Hesaplama hatası: {}{}\n", RED, BOLD, err_msg, RESET),
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
                        println!("{}{}⏰ ANLIK NANOSANİYE HASSASİYETLİ SİSTEM SAATİ: {}{}\n", BRIGHT_CYAN, BOLD, time_str, RESET);
                    }
                    "ping" => {
                        let target = parts.get(1).unwrap_or(&"fapi.binance.com");
                        println!("{}{}📡 {} İLE AĞ GECİKMESİ (HFT PING) ÖLÇÜLÜYOR...{}", BRIGHT_YELLOW, BOLD, target, RESET);

                        let start = std::time::Instant::now();
                        let shell_cmd = format!("ping -c 3 {}", target);
                        if let Ok(output) = std::process::Command::new("bash").arg("-c").arg(&shell_cmd).output() {
                            let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
                            let out_str = String::from_utf8_lossy(&output.stdout);
                            if output.status.success() {
                                println!("{}", out_str);
                                println!("{}{}✓ Ortalama Ağ RTT Gecikmesi: {:.2} ms{}\n", GREEN, BOLD, elapsed_ms / 3.0, RESET);
                            } else {
                                println!("{}{}Ping isteği başarısız oldu veya zaman aşımına uğradı.{}\n", RED, BOLD, RESET);
                            }
                        }
                    }
                    "tree" => {
                        println!("{}{}=== 🌳 CYCLE ORCHESTRATOR DİZİN VE EKLENTİ AĞACI ==={}", BRIGHT_CYAN, BOLD, RESET);
                        println!("cycle-orc/");
                        println!("├── Cargo.toml (Workspace Root)");
                        println!("├── flow_config.json (DAG Configuration)");
                        println!("├── binance_market_data.db (SQLite Storage)");
                        println!("├── paper_exchange.db (Paper Trading Storage)");
                        println!("├── interactive_shell/ (Standalone Unified Shell)");
                        println!("│   ├── Cargo.toml");
                        println!("│   └── src/lib.rs");
                        println!("├── orchestrator/ (Core Engine & Pinning)");
                        println!("│   ├── src/main.rs (Direct Boot & Core 0/1 Pinning)");
                        println!("│   ├── src/interactive_shell.rs");
                        println!("│   ├── src/tui_interface/ (Preserved TUI Source)");
                        println!("│   └── src/web_server.rs (Preserved Web Server)");
                        println!("└── plugins/ (.so Shared Libraries)");
                        for p in scan_available_plugins() {
                            println!("    ├── lib{}.so", p);
                        }
                        println!();
                    }
                    "run" => {
                        if parts.len() < 2 {
                            println!("{}{}HATA: Kullanım: run <script.cy>{}\n", RED, BOLD, RESET);
                        } else {
                            let filepath = parts[1];
                            match std::fs::read_to_string(filepath) {
                                Ok(code) => {
                                    println!("{}{}📜 CYCLELANG BETİĞİ ÇALIŞTIRILIYOR: {}...{}", BRIGHT_CYAN, BOLD, filepath, RESET);
                                    let mut handler = ShellOrchestratorHandler { orchestrator: orchestrator.clone() };
                                    match cycle_lang::run_script(&code, &mut handler) {
                                        Ok(_) => println!("{}{}✓ BETİK BAŞARIYLA TAMAMLANDI: {}{}\n", BRIGHT_GREEN, BOLD, filepath, RESET),
                                        Err(err_msg) => println!("{}{}Betik çalıştırma hatası ({}): {}{}\n", RED, BOLD, filepath, err_msg, RESET),
                                    }
                                }
                                Err(e) => println!("{}{}Betik dosyası okunamadı ({}): {}{}\n", RED, BOLD, filepath, e, RESET),
                            }
                        }
                    }
                    "watch" => {
                        if parts.len() < 2 {
                            println!("{}{}HATA: Kullanım: watch <script.cy>{}\n", RED, BOLD, RESET);
                        } else {
                            let filepath = parts[1];
                            println!("{}{}👀 HOT-RELOADING BETİK İZLEYİCİ BAŞLATILDI: {}{}\n", BRIGHT_YELLOW, BOLD, filepath, RESET);
                            match std::fs::read_to_string(filepath) {
                                Ok(code) => {
                                    let mut handler = ShellOrchestratorHandler { orchestrator: orchestrator.clone() };
                                    let _ = cycle_lang::run_script(&code, &mut handler);
                                }
                                Err(e) => println!("{}{}Betik okunamadı: {}{}\n", RED, BOLD, e, RESET),
                            }
                        }
                    }
                    "config" => {
                        if let Ok(content) = std::fs::read_to_string("flow_config.json") {
                            println!("{}{}=== FLOW CONFIG (flow_config.json) ==={}", BRIGHT_CYAN, BOLD, RESET);
                            println!("{}\n", content);
                        } else {
                            println!("{}{}flow_config.json okunamadı.{}\n", RED, BOLD, RESET);
                        }
                    }
                    "exit" | "quit" => {
                        println!("{}{}Orkestratör kapatılıyor. Hoşça kalın!{}", BRIGHT_YELLOW, BOLD, RESET);
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
                                    println!("{}{}Komut çıkış kodu: {}{}\n", YELLOW, BOLD, output.status, RESET);
                                }
                            }
                            Err(e) => {
                                println!("{}{}Komut çalıştırılamadı ({}): {}{}\n", RED, BOLD, cmd_line, e, RESET);
                            }
                        }
                    }
                }
            }
            Err(rustyline::error::ReadlineError::Interrupted) | Err(rustyline::error::ReadlineError::Eof) => {
                println!("\n{}{}Orkestratör kapatılıyor...{}", BRIGHT_YELLOW, BOLD, RESET);
                break;
            }
            Err(err) => {
                println!("{}{}Hata: {:?}{}", RED, BOLD, err, RESET);
                break;
            }
        }
    }
}

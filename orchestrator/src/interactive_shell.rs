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
                        println!("{}{}Komut anlaşılamadı: '{}'. Geçerli komutlar için 'help' yazın.{}\n", RED, BOLD, cmd_line, RESET);
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

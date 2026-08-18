use std::path::PathBuf;

pub const RESET: &str = "\x1b[0m";
pub const BOLD: &str = "\x1b[1m";
pub const RED: &str = "\x1b[31m";
pub const GREEN: &str = "\x1b[32m";
pub const YELLOW: &str = "\x1b[33m";
pub const BLUE: &str = "\x1b[34m";
pub const MAGENTA: &str = "\x1b[35m";
pub const CYAN: &str = "\x1b[36m";
pub const WHITE: &str = "\x1b[37m";
pub const GRAY: &str = "\x1b[90m";
pub const BRIGHT_GREEN: &str = "\x1b[92m";
pub const BRIGHT_CYAN: &str = "\x1b[96m";
pub const BRIGHT_YELLOW: &str = "\x1b[93m";

pub fn print_banner() {
    println!("{}{}╔══════════════════════════════════════════════════════════════════════════════╗{}", BRIGHT_CYAN, BOLD, RESET);
    println!("{}{}║   🚀 CYCLE ORCHESTRATOR - HIGH FREQUENCY UNIFIED COMMAND SHELL               ║{}", BRIGHT_CYAN, BOLD, RESET);
    println!("{}{}╚══════════════════════════════════════════════════════════════════════════════╝{}", BRIGHT_CYAN, BOLD, RESET);
    println!("{}Komut listesini görmek için '{}{}{}help{}{}' yazın. Çıkmak için '{}{}{}exit{}{}' veya '{}{}{}quit{}{}' yazın.{}\n", 
        GRAY, RESET, BRIGHT_YELLOW, BOLD, RESET, GRAY, RESET, RED, BOLD, RESET, GRAY, RESET, RED, BOLD, RESET, GRAY, RESET);
}

pub fn get_plugin_dir() -> PathBuf {
    let mut dir = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    dir.pop();
    if dir.ends_with("deps") {
        dir.pop();
    }
    dir
}

pub fn scan_available_plugins() -> Vec<String> {
    let mut plugins = Vec::new();
    let lib_dir = get_plugin_dir();
    let prefix = if cfg!(target_os = "windows") { "" } else { "lib" };
    if let Ok(entries) = std::fs::read_dir(&lib_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with(&format!("{}plugin_", prefix)) && (name.ends_with(".so") || name.ends_with(".dll") || name.ends_with(".dylib")) {
                let ext_len = if name.ends_with(".so") { 3 } else if name.ends_with(".dll") { 4 } else { 6 };
                let plugin_name = &name[prefix.len()..name.len()-ext_len];
                plugins.push(plugin_name.to_string());
            }
        }
    }
    plugins.sort();
    plugins.dedup();
    plugins
}

pub fn format_help_menu() -> String {
    let mut out = String::new();
    out.push_str(&format!("{}{}=== CYCLE ORCHESTRATOR UNIFIED SHELL KOMUT KILAVUZU ==={}\n", BRIGHT_CYAN, BOLD, RESET));
    
    out.push_str(&format!("\n{}{}⚙️ SİSTEM VE EKLENTİ YÖNETİMİ:{}\n", BRIGHT_YELLOW, BOLD, RESET));
    out.push_str(&format!("  {}help{}                   : Bu yardım menüsünü görüntüler\n", GREEN, RESET));
    out.push_str(&format!("  {}list{}                   : Yüklü tüm eklentileri, durumlarını, RAM (KB) ve CPU (%) kullanımını basar\n", GREEN, RESET));
    out.push_str(&format!("  {}available{}              : Diskte derlenmiş yüklemeye hazır eklenti (.so) kütüphanelerini listeler\n", GREEN, RESET));
    out.push_str(&format!("  {}status [plugin_id]{}     : Genel sistem metriklerini veya spesifik eklenti detayını gösterir\n", GREEN, RESET));
    out.push_str(&format!("  {}metrics{}                : Detaylı CPU, RAM bellek ve isolated core istatistiklerini görüntüler\n", GREEN, RESET));
    out.push_str(&format!("  {}start <id|all>{}         : Belirtilen eklentiyi veya tüm sistemi başlatır\n", GREEN, RESET));
    out.push_str(&format!("  {}stop <id|all>{}          : Belirtilen eklentiyi veya tüm sistemi durdurur\n", GREEN, RESET));
    out.push_str(&format!("  {}load <plugin_name>{}     : C-ABI dynamic library (.so) kütüphanesini anında yükler\n", GREEN, RESET));
    out.push_str(&format!("  {}del <plugin_id>{}        : Eklentiyi hafızadan tamamen kaldırır\n", GREEN, RESET));

    out.push_str(&format!("\n{}{}📈 CANLI PİYASA TELEMETRİ SORGULARI:{}\n", BRIGHT_YELLOW, BOLD, RESET));
    out.push_str(&format!("  {}fetch ticker <symbol>{}   : Sembolün canlı en iyi alış/satış (Best Bid/Ask) ve spread verisini sorgular\n", GREEN, RESET));
    out.push_str(&format!("  {}fetch depth <symbol>{}    : Orderbook (derinlik) canlı snapshot verisini gösterir\n", GREEN, RESET));
    out.push_str(&format!("  {}fetch oi <symbol>{}       : Açık pozisyon (Open Interest) verisini çekip görüntüler\n", GREEN, RESET));
    out.push_str(&format!("  {}fetch ohlcv <symbol>{}    : Canlı mum (Open, High, Low, Close, Volume) verilerini listeler\n", GREEN, RESET));

    out.push_str(&format!("\n{}{}💾 VERİTABANI VE PAPER TRADING:{}\n", BRIGHT_YELLOW, BOLD, RESET));
    out.push_str(&format!("  {}tables{}                 : SQLite veritabanındaki tüm tabloları ve kayıt sayılarını gösterir\n", GREEN, RESET));
    out.push_str(&format!("  {}schema <table>{}          : Tablo yapısını ve sütun tiplerini görüntüler\n", GREEN, RESET));
    out.push_str(&format!("  {}sql <query>{}            : Doğrudan SQL sorgusu çalıştırır (örn: sql SELECT * FROM mark_prices LIMIT 5)\n", GREEN, RESET));
    out.push_str(&format!("  {}buy <sym> <qty> <price>{} : Sanal alım (Long) emri girer\n", GREEN, RESET));
    out.push_str(&format!("  {}sell <sym> <qty> <price>{}: Sanal satım (Short) emri girer\n", GREEN, RESET));
    out.push_str(&format!("  {}positions{}              : Açık paper trading pozisyonlarını ve PnL durumunu listeler\n", GREEN, RESET));

    out.push_str(&format!("\n{}{}🛠️ KONTROL VE APİ DÜZEYİ:{}\n", BRIGHT_YELLOW, BOLD, RESET));
    out.push_str(&format!("  {}peek <plugin_id> [len]{}  : Eklentinin RAM bellek tamponunu (Hex & ASCII) inceler\n", GREEN, RESET));
    out.push_str(&format!("  {}web <start|stop|status>{} : Port 8080 Web sunucusunu başlatır, durdurur veya durumunu verir\n", GREEN, RESET));
    out.push_str(&format!("  {}config [show|reload]{}   : flow_config.json içeriğini okur veya hot-reload tetikler\n", GREEN, RESET));
    out.push_str(&format!("  {}clear{}                  : Ekranı temizler\n", GREEN, RESET));
    out.push_str(&format!("  {}exit{} / {}quit{}            : Orkestratörü kapatır ve kabuktan çıkar\n", RED, RESET, RED, RESET));

    out
}

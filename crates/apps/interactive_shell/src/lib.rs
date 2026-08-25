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
    println!("{}Type '{}{}{}help{}{}' to see command list. Type '{}{}{}exit{}{}' or '{}{}{}quit{}{}' to exit.{}\n", 
        GRAY, RESET, BRIGHT_YELLOW, BOLD, RESET, GRAY, RESET, RED, BOLD, RESET, GRAY, RESET, RED, BOLD, RESET, GRAY, RESET);
}

pub fn get_plugin_dir() -> PathBuf {
    let mut dir = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    dir.pop();
    if dir.ends_with("deps") {
        dir.pop();
    }
    if dir.join("target/debug").exists() {
        return dir.join("target/debug");
    }
    if std::path::Path::new("target/debug").exists() {
        return PathBuf::from("target/debug");
    }
    dir
}

pub fn scan_available_plugins() -> Vec<String> {
    let mut plugins = Vec::new();
    let search_dirs = vec![
        get_plugin_dir(),
        PathBuf::from("target/debug"),
        PathBuf::from("../target/debug"),
        PathBuf::from("."),
    ];

    let prefix = if cfg!(target_os = "windows") { "" } else { "lib" };

    for lib_dir in search_dirs {
        if let Ok(entries) = std::fs::read_dir(&lib_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                let is_lib = name.ends_with(".so") || name.ends_with(".dll") || name.ends_with(".dylib");
                if is_lib && !name.contains(".d") {
                    let ext_len = if name.ends_with(".so") { 3 } else if name.ends_with(".dll") { 4 } else { 6 };
                    let clean_name = if name.starts_with(prefix) {
                        &name[prefix.len()..name.len()-ext_len]
                    } else {
                        &name[..name.len()-ext_len]
                    };
                    if !clean_name.is_empty() {
                        plugins.push(clean_name.to_string());
                    }
                }
            }
        }
    }
    plugins.sort();
    plugins.dedup();
    plugins
}

pub fn format_help_menu() -> String {
    let mut out = String::new();
    out.push_str(&format!("{}{}=== CYCLE ORCHESTRATOR UNIFIED SHELL COMMAND GUIDE ==={}\n", BRIGHT_CYAN, BOLD, RESET));
    
    out.push_str(&format!("\n{}{}⚙️ SYSTEM AND PLUGIN MANAGEMENT:{}\n", BRIGHT_YELLOW, BOLD, RESET));
    out.push_str(&format!("  {}help{}                   : Displays this help menu\n", GREEN, RESET));
    out.push_str(&format!("  {}list{}                   : Displays all loaded plugins, status, RAM (KB) and CPU (%) usage\n", GREEN, RESET));
    out.push_str(&format!("  {}available{}              : Lists compiled plugin (.so) libraries ready for loading on disk\n", GREEN, RESET));
    out.push_str(&format!("  {}status [plugin_id]{}     : Shows general system metrics or specific plugin details\n", GREEN, RESET));
    out.push_str(&format!("  {}metrics{}                : Displays detailed CPU, RAM memory and isolated core statistics\n", GREEN, RESET));
    out.push_str(&format!("  {}start <id|all>{}         : Starts specified plugin or the entire system\n", GREEN, RESET));
    out.push_str(&format!("  {}stop <id|all>{}          : Stops specified plugin or the entire system\n", GREEN, RESET));
    out.push_str(&format!("  {}load <plugin_name>{}     : Dynamically loads C-ABI dynamic library (.so) instantly\n", GREEN, RESET));
    out.push_str(&format!("  {}del <plugin_id>{}        : Completely unloads plugin from memory\n", GREEN, RESET));

    out.push_str(&format!("\n{}{}📈 LIVE MARKET TELEMETRY QUERIES:{}\n", BRIGHT_YELLOW, BOLD, RESET));
    out.push_str(&format!("  {}fetch ticker <symbol>{}   : Queries live Best Bid/Ask and spread data for symbol\n", GREEN, RESET));
    out.push_str(&format!("  {}fetch depth <symbol>{}    : Shows live Orderbook depth snapshot data\n", GREEN, RESET));
    out.push_str(&format!("  {}fetch oi <symbol>{}       : Fetches and displays Open Interest data\n", GREEN, RESET));
    out.push_str(&format!("  {}fetch ohlcv <symbol>{}    : Lists live candlestick (Open, High, Low, Close, Volume) data\n", GREEN, RESET));
    out.push_str(&format!("  {}fetch amihud <symbol>{}   : Displays Amihud Illiquidity Ratio (liquidity/volume sensitivity) analysis\n", GREEN, RESET));

    out.push_str(&format!("\n{}{}💾 DATABASE AND PAPER TRADING:{}\n", BRIGHT_YELLOW, BOLD, RESET));
    out.push_str(&format!("  {}tables{}                 : Displays all tables and record counts in SQLite database\n", GREEN, RESET));
    out.push_str(&format!("  {}schema <table>{}          : Displays table structure and column types\n", GREEN, RESET));
    out.push_str(&format!("  {}sql <query>{}            : Executes raw SQL query (e.g. sql SELECT * FROM mark_prices LIMIT 5)\n", GREEN, RESET));
    out.push_str(&format!("  {}buy <sym> <qty> <price> [lev]{}: Enters paper buy (Long) order (e.g. buy BTCUSDT 0.1 60000 20)\n", GREEN, RESET));
    out.push_str(&format!("  {}sell <sym> <qty> <price> [lev]{}: Enters paper sell (Short) order (e.g. sell ETHUSDT 1.5 3000 50)\n", GREEN, RESET));
    out.push_str(&format!("  {}cancel <order_id>{}       : Cancels pending order\n", GREEN, RESET));
    out.push_str(&format!("  {}cancelall [symbol]{}     : Cancels all pending orders or symbol-specific orders\n", GREEN, RESET));
    out.push_str(&format!("  {}deposit <amount>{}       : Deposits virtual balance (e.g. deposit 5000)\n", GREEN, RESET));
    out.push_str(&format!("  {}setbalance <amount>{}    : Directly sets paper wallet balance (e.g. setbalance 10000)\n", GREEN, RESET));
    out.push_str(&format!("  {}positions{}              : Lists open paper trading positions and PnL status\n", GREEN, RESET));
    out.push_str(&format!("  {}orders [symbol]{}        : Lists active pending orders\n", GREEN, RESET));
    out.push_str(&format!("  {}history [limit]{}         : Lists closed trade history with PnL and entry/exit prices\n", GREEN, RESET));
    out.push_str(&format!("  {}close <symbol>{}          : Closes open symbol position\n", GREEN, RESET));
    out.push_str(&format!("  {}closeall{}               : Instantly closes all open positions via market order\n", GREEN, RESET));

    out.push_str(&format!("\n{}{}⚡ HFT BENCHMARK AND TOPOLOGY:{}\n", BRIGHT_YELLOW, BOLD, RESET));
    out.push_str(&format!("  {}bench [iterations]{}     : Measures Zero-copy C-ABI endpoint latency in nanoseconds (ns)\n", GREEN, RESET));
    out.push_str(&format!("  {}graph{} / {}routes{}         : Visualizes Flow Engine Directed Acyclic Graph (DAG)\n", GREEN, RESET, GREEN, RESET));

    out.push_str(&format!("\n{}{}🖥️ OPERATING SYSTEM AND PC SHELL COMMANDS:{}\n", BRIGHT_YELLOW, BOLD, RESET));
    out.push_str(&format!("  {}cd <path>{}              : Changes working directory (e.g. cd /home)\n", GREEN, RESET));
    out.push_str(&format!("  {}pwd{}                    : Displays current working directory\n", GREEN, RESET));
    out.push_str(&format!("  {}sysinfo{} / {}pc{}           : Shows OS, kernel, CPU model, RAM and disk usage\n", GREEN, RESET, GREEN, RESET));
    out.push_str(&format!("  {}<local commands>{}         : Directly executes Linux/OS commands (e.g. ls, free -h, df -h, git status, ping)\n", GREEN, RESET));

    out.push_str(&format!("\n{}{}🧮 CALCULATION AND FINANCIAL UTILITIES:{}\n", BRIGHT_YELLOW, BOLD, RESET));
    out.push_str(&format!("  {}calc <expression>{}        : Performs mathematical/financial calculations (e.g. calc 65000 * 0.1 * 20)\n", GREEN, RESET));
    out.push_str(&format!("  {}time{} / {}clock{}           : Displays nanosecond-precision live system clock (HH.MM.SS.mmm.uuu.nnn)\n", GREEN, RESET, GREEN, RESET));
    out.push_str(&format!("  {}ping [target]{}           : Measures Binance HFT API network latency (RTT ms)\n", GREEN, RESET));
    out.push_str(&format!("  {}tree{}                    : Visualizes project plugin and module directory tree\n", GREEN, RESET));

    out.push_str(&format!("\n{}{}🛠️ CONTROL AND API LEVEL:{}\n", BRIGHT_YELLOW, BOLD, RESET));
    out.push_str(&format!("  {}dump <plugin_id> [max_bytes]{}: Dumps plugin's FULL RAM buffer (Full Memory Hex Dump & ASCII)\n", GREEN, RESET));
    out.push_str(&format!("  {}exportjson <id> [file]{}  : Saves plugin's memory JSON output as a .json file (Aliases: dumpjson, savejson)\n", GREEN, RESET));
    out.push_str(&format!("  {}peek <plugin_id> [len]{}  : Inspects plugin's initial RAM memory summary\n", GREEN, RESET));
    out.push_str(&format!("  {}web <start|stop|status>{} : Starts, stops or returns status of Port 8080 Web server\n", GREEN, RESET));
    out.push_str(&format!("  {}config [show|reload]{}   : Reads flow_config.json content or triggers hot-reload\n", GREEN, RESET));
    out.push_str(&format!("  {}clear{}                  : Clears screen\n", GREEN, RESET));
    out.push_str(&format!("  {}exit{} / {}quit{}            : Stops orchestrator and exits shell\n", RED, RESET, RED, RESET));

    out
}

use interactive_shell::*;

fn main() {
    print_banner();
    println!("==========================================================================================");
    println!("🚀 CYCLE ORCHESTRATOR INTERACTIVE SHELL STANDALONE RUNNER");
    println!("==========================================================================================");
    println!("Full Orchestrator & Live HFT Command: cargo run -p cycle-finance-breakout-system\n");

    let plugins = scan_available_plugins();
    if plugins.is_empty() {
        println!("No compiled dynamic plugins (.so) found in target/debug. Compile plugins via: cargo build\n");
    } else {
        println!("Compiled plugins available on disk:");
        for p in plugins {
            println!("  • {}", p);
        }
        println!();
    }
}

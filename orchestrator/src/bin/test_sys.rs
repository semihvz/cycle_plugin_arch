use sysinfo::System;
use std::process;

fn main() {
    let mut sys = System::new_all();
    let pid = sysinfo::Pid::from_u32(process::id());
    sys.refresh_processes();
    
    if let Some(p) = sys.process(pid) {
        println!("CPU: {}", p.cpu_usage());
        println!("MEM: {}", p.memory());
    }
}

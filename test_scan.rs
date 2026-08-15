use std::path::PathBuf;

fn main() {
    let mut lib_dir = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    lib_dir.pop();
    let prefix = if cfg!(target_os = "windows") { "" } else { "lib" };
    println!("Scanning directory: {:?}", lib_dir);
    if let Ok(entries) = std::fs::read_dir(&lib_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with(&format!("{}plugin_", prefix)) && (name.ends_with(".so") || name.ends_with(".dll") || name.ends_with(".dylib")) {
                let ext_len = if name.ends_with(".so") { 3 } else if name.ends_with(".dll") { 4 } else { 6 };
                let plugin_name = &name[prefix.len()..name.len()-ext_len];
                println!("Found plugin: {}", plugin_name);
            }
        }
    } else {
        println!("Failed to read dir");
    }
}

use std::path::PathBuf;
fn main() {
    let mut lib_dir = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    lib_dir.pop();
    let prefix = if cfg!(target_os = "windows") { "" } else { "lib" };
    println!("Scanning: {:?}", lib_dir);
    for entry in std::fs::read_dir(&lib_dir).unwrap().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.contains("plugin_") {
            println!("File: {}", name);
        }
    }
}

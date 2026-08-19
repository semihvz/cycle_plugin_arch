// ============================================================================
// util — tek örnek koruması ve port bağlama
// ============================================================================

use std::fs;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process;

use tokio::net::TcpListener;

/// İkiz süreç koruması. Aynı isimde zaten çalışan bir süreç varsa
/// ikinci süreç çıkar. Ölü süreç kalıntısı (PID /proc'da yoksa) temizlenir.
pub fn single_instance(name: &str) -> Result<(), String> {
    let lock_path = PathBuf::from(format!("/tmp/{name}.lock"));

    if lock_path.exists() {
        let pid = fs::read_to_string(&lock_path)
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok());
        if let Some(pid) = pid {
            if process_alive(pid) {
                eprintln!("[{name}] İkiz süreç tespit edildi (PID {pid}), çıkılıyor.");
                process::exit(1);
            }
        }
        // Ölü süreç kalıntısı — temizle ve devam et
        let _ = fs::remove_file(&lock_path);
    }

    fs::write(&lock_path, process::id().to_string()).map_err(|e| e.to_string())
}

fn process_alive(pid: u32) -> bool {
    std::path::Path::new(&format!("/proc/{pid}")).exists()
}

/// Adresi bağla; başarısızlıkta hata mesajı ile temiz çıkış yap.
pub async fn bind_or_exit(addr: SocketAddr, name: &str) -> TcpListener {
    match TcpListener::bind(addr).await {
        Ok(listener) => listener,
        Err(e) => {
            eprintln!("[{name}] Port {} bağlanamadı: {}", addr.port(), e);
            process::exit(1);
        }
    }
}

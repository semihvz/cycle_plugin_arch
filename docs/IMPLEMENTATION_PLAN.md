# HFT (Ultra Düşük Gecikmeli) Mimari Geçiş Planı

Ana sistemi (Orkestratör) standart bir yönetim aracından çıkarıp, mikrosaniye/nanosaniye seviyesinde rekabet edebilecek **Gerçek Zamanlı bir HFT Motoruna** dönüştürmek için kapsamlı bir mimari değişiklik yapılmıştır.

> ⚠️ **Kritik Değişiklik Uyarısı (Breaking Change)**
> Bu geçiş, orkestratörün çekirdek `System` arayüzünü (Trait) ve bellek yönetimini tamamen değiştirmiştir. Bundan sonra yazılacak olan tüm yeni eklentilerin (pluginlerin), bu yeni "Sıfır Kopyalama" (Zero-copy) ve "V-Table'sız" (C-ABI) kurallarına göre geliştirilmesi gerekmektedir.

## ❓ Karar Verilen Konular
- ✅ Yeni eklentiler (pluginler) oluştururken artık Rust `dyn System` trait'i yerine, doğrudan C-ABI stili `extern "C"` metodlar dışa aktarılacak. Bu HFT için en iyi yoldur.
- ✅ CPU Sabitleme (Pinning) işlemi için 2 çekirdek kısıtı uygulanacak. Ana döngü Çekirdek 0'a sabitlenmiştir.

## 🛠 Yapılan Değişiklikler

Aşağıdaki bileşenler HFT mimarisine (Lock-free, Zero-copy, No V-Table, CPU Pinning) uygun şekilde tamamen yeniden yazılmıştır:

---

### Orkestratör Çekirdek Yapıları

#### [MODIFY] memory.rs
- **Kaldırılanlar:** `Arc<RwLock<Vec<u8>>>` mekanizması.
- **Eklenenler:** 
  - Gelen ve giden mesajlar (Inbox/Outbox) için `crossbeam::queue::ArrayQueue` tabanlı **Lock-Free Ring Buffer**. Thread'ler kilitlenmeden okuma/yazma yapabilecek.

#### [MODIFY] system.rs
- **Kaldırılanlar:** `dyn System` tabanlı dinamik dağıtım (V-Table gecikmesi).
- **Eklenenler:**
  - `SystemInstance` isimli yeni bir struct. Eklentinin RAM'deki referans adresleri (`*mut c_void`) ve fonksiyonları (`extern "C" fn(payload: *const u8, len: usize)`) ham fonksiyon pointer'ları (Raw Function Pointers) olarak saklanmaktadır. Böylece `call()` atıldığı an araya sanal fonksiyon tablosu girmeden direkt CPU komutu çalıştırılır.
  - Eklenti içi veriler (Payload) kopyalama gerektiren `Vec<u8>` yerine `&[u8]` pointer referanslarına çevrilmiştir (Zero-copy).
  - Durum bilgileri (`is_running`, `is_data_valid`) `Arc<RwLock<bool>>` yerine `Arc<AtomicBool>` ile lock-free takip edilmektedir.

#### [MODIFY] endpoint.rs
- **Eklenenler:** `#[repr(u32)]` ile C-ABI uyumlu bellek düzeni (FFI uyumlu enum). Her endpoint sabit bir tamsayı değerine sahiptir.

#### [MODIFY] orchestrator.rs
- **Kaldırılanlar:** Sisteme ağır yük bindiren `DashMap` kullanımı.
- **Eklenenler:**
  - Eklentiler `Vec<Arc<SystemInstance>>` içinde saklanmaktadır.
  - Rota (Routing) çağrıları optimize edilmiştir; `payload` aktarımı sıfır kopyalama (Zero-copy) ile `&[u8]` referans dilimi olarak yapılmaktadır.

#### [MODIFY] main.rs
- **Kaldırılanlar:** İşletim sisteminin thread scheduling (zamanlama) insiyatifine bırakılan iş akışları. Eski `create_plugin` (Box<dyn System>) eklenti yükleme sistemi.
- **Eklenenler:** 
  - `core_affinity` kütüphanesi ile ana thread CPU Çekirdek 0'a sabitlenmiştir (CPU Pinning). L1/L2 önbellek silinmeleri (Cache Misses) engellenmiştir.
  - Yeni `init_plugin` C-ABI eklenti yükleme sistemi (`extern "C" fn(state_out) -> RawEndpointFn`).
  - 1MB pre-allocated `hft_buf` buffer'ı ile sıcak yolda sıfır heap allokasyonu sağlanmıştır.

---

## 🧪 Doğrulama Sonuçları (Verification Results)
1. ✅ **Derleme:** `cargo check` ve `cargo build` komutlarıyla HFT paketleri (`crossbeam`, `core_affinity`) başarıyla entegre edildi. Sıfır hata, sıfır uyarı.
2. ✅ **Commit ve Push:** Tüm değişiklikler GitHub'a başarıyla pushlandı.

## 📊 Önce / Sonra Karşılaştırması

| Bileşen | Eski (Darboğaz) | Yeni (HFT) |
|---|---|---|
| **memory.rs** | `Arc<RwLock<Vec<u8>>>` (Lock) | `crossbeam::ArrayQueue` (Lock-free Ring Buffer) |
| **system.rs** | `Box<dyn System>` + V-Table | `SystemInstance` + `extern "C"` raw fn pointers |
| **endpoint.rs** | Rust enum | `#[repr(u32)]` C-ABI enum |
| **orchestrator.rs** | `DashMap` (ağır Lock) | `Vec<Arc<SystemInstance>>` + zero-copy `&[u8]` |
| **main.rs (Eklenti)** | `create_plugin` → `Box<dyn System>` | `init_plugin` → C-ABI fonksiyon pointer |
| **main.rs (CPU)** | İşletim sistemi thread scheduling | `core_affinity` ile CPU Core 0'a sabitleme |
| **main.rs (Bellek)** | Her çağrıda yeni `Vec<u8>` allokasyonu | 1MB pre-allocated `hft_buf` |

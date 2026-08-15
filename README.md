# Cycle Orchestrator - Ana Sistem (Core) Mimarisi

Ana sistem (`orchestrator`), diğer tüm iş mantıklarından (plugin'lerden) tamamen arındırılmış, sadece "yönetim", "arayüz" ve "RAM tabanlı iletişim" altyapısını kuran merkezi bir kabuk (shell) olarak tasarlanmıştır.

Aşağıda ana sistemin mimari bileşenleri detaylandırılmıştır:

## 1. Temel Kavram (The Core Concept)
Ana sistem kendi başına hiçbir ticaret mantığı, ağ isteği veya veritabanı işlemi içermez. Amacı; dinamik kütüphane (`.so` / `.dll`) olarak derlenmiş eklentileri çalışma zamanında (runtime) belleğe yüklemek, bunları yönetmek ve eklentiler arası gecikmesiz (RAM üzerinden) haberleşmeyi sağlamaktır.

## 2. Modüller ve Görevleri

### `orchestrator.rs` (Merkezi Yönetici)
Tüm sistemin kalbidir. 
- **DashMap Kullanımı:** `systems: DashMap<String, SystemBox>` veri yapısıyla, yüklü eklentileri (sistemleri) thread-safe ve eşzamanlı bir şekilde RAM'de tutar.
- **Görevleri:** Eklenti yükleme (`register_system`), eklenti silme (`unregister_system`) ve eklentilerin fonksiyonlarını (endpoint) çağırma işlemlerini asenkron bloklamalar olmadan doğrudan bellek referanslarıyla gerçekleştirir.

### `system.rs` (Eklenti Sözleşmesi)
Tüm eklentilerin uyması gereken katı arayüzdür (Trait).
- **`System` Trait'i:** Eklentilerin kimliğini, portlarını, endpoint'lerini ve bellek bağlamını (`context`) dışarıya sunmasını zorunlu kılar.
- **`SystemContext`:** Her eklenti için oluşturulan özel veri alanıdır. Eklentinin adı, çalışma durumu (`is_running`), verisinin geçerliliği (`is_data_valid`) ve kendisine tahsis edilmiş olan ayrılmış RAM alanı (`memory`) bu bağlam içinde yer alır.
- **Dinamik Dağıtım (Dynamic Dispatch):** `Box<dyn System>` kullanılarak eklentinin ne iş yaptığı bilinmeden standart bir nesne gibi kontrol edilmesi sağlanır.

### `endpoint.rs` (Standart İletişim Protokolü)
Eklentiler ve Orkestratör arasındaki ortak dildir. RPC (Remote Procedure Call) gibi çalışır ama ağ üzerinden değil RAM üzerinden tetiklenir.
- **Temel Endpoint'ler:** `Start` (başlat), `Stop` (durdur), `DataMonitor` (izleme), `Inbox` (mesaj alma), `Outbox` (mesaj gönderme) vb.
- Yeni bir eklenti yüklendiğinde, orkestratör bu endpoint'lere veriler yollayarak eklentiyi yönetir.

### `memory.rs` (Gecikmesiz Veri Alanı)
Network socket'leri veya dosyalar yerine sistemlerin sıfır gecikmeyle veri alışverişi yapabilmesi için tasarlanmıştır.
- `Arc<RwLock<Vec<u8>>>` yapısını kullanır.
- Büyük veri blokları (örneğin devasa orderbook verileri) serileştirilip byte dizisi olarak burada tutulur ve orkestratör tarafından anında okunur/yazılır.

## 3. Dinamik Eklenti Yükleme (libloading)
Ana sistem, derleme anında eklentilere ihtiyaç duymaz (Hardcoded bağımlılık yoktur).
Çalışma anında `target/debug/` klasöründeki `libplugin_*.so` (Linux) veya `*.dll` (Windows) dosyalarını tarar.
Kullanıcı bir eklentiyi seçtiğinde `libloading::Library::new()` ile kütüphaneyi belleğe alır ve `create_plugin` sembolünü çağırarak eklenti nesnesini ayağa kaldırıp sisteme bağlar (`register_system`).

## 4. Terminal Arayüzü (TUI)
`main.rs` ve `tui.rs` dosyalarında barınır.
- **Ratatui & Crossterm:** Konsol tabanlı, hafif ve modern bir arayüz çizer.
- **Event Loop (Olay Döngüsü):** Klavyeden basılan tuşları veya fareden gelen tıklama (MouseCapture) koordinatlarını dinleyerek eklentileri (seçme, başlatma, durdurma, izleme, silme) arayüz üzerinden yönetmeye imkan tanır.
- Her arayüz yenilenmesinde arka planda `DataMonitor` endpoint'i üzerinden seçili sistemin verilerini okuyarak canlı olarak ekrana basar.

## 5. İletişim ve Veri Akışı
1. Kullanıcı arayüzden "Başlat"a tıklar.
2. TUI, Orkestratörün `call_endpoint(id, StandardEndpoint::Start)` fonksiyonunu çağırır.
3. Orkestratör, bellekteki (DashMap) eklenti nesnesini bulur ve doğrudan onun RAM'indeki fonksiyona bağlanıp işlemi yürütür.
4. Ağ soketi oluşturulmaz, API kullanılmaz. Bu nedenle işlemler **nanosaniye/mikrosaniye** seviyesinde tamamlanır.

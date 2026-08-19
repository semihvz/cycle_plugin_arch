# Node-Tabanlı Veri Akış ve Yönlendirme (Data Router) Mimarisi

Kullanıcı gereksinimleri doğrultusunda, eklentiler arası haberleşmeyi "ana orkestratörden (TUI/Sistem Yönetimi)" ayırarak, **tamamen bağımsız ve konfigürasyon (ayar dosyası) odaklı yeni bir Çekirdek Sistem (Data Router / Flow Engine)** tasarlanmıştır.

## 1. Temel Felsefe: Eklentiler = Fonksiyonlar (Düğümler)
Her eklenti (plugin) girdisi (Input) ve çıktısı (Output) olan bağımsız bir fonksiyon / kara kutu olarak tasarlanacaktır. 
Eklenti kimden veri aldığını veya kime veri gönderdiğini bilmez; sadece kendisine verilen **Girdi Tamponunu (Input Buffer)** okur ve kendi **Çıktı Tamponuna (Output Buffer)** yazar.

## 2. Konfigürasyon Dosyası ile Dinamik Bağlantı (Routing)
Hangi eklentinin çıktısının, hangi eklentinin girdisine bağlanacağı koda gömülmeyecektir (Hardcoded olmayacak). Bu yapı, kullanıcı tarafından bir `flow_config.toml` (veya json/yaml) dosyası ile belirlenecektir.

**Örnek `flow_config.toml` Yapısı:**
```toml
# VERİ ÜRETİCİLERİ (Kaynaklar)
[[plugin]]
name = "all_markprices"
type = "producer"
outputs = ["stream_markprice"]

[[plugin]]
name = "all_aggtrades"
type = "producer"
outputs = ["stream_trades"]

# VERİ TÜKETİCİLERİ / ANALİZÖRLER
[[plugin]]
name = "ms_analyzer"
type = "processor"
inputs = { markprice = "stream_markprice", trades = "stream_trades" }
outputs = ["stream_ms_signals"]

# KARAR VERİCİ / İŞLEM (Tüketici)
[[plugin]]
name = "plugin_breakout"
type = "consumer"
inputs = { signals = "stream_ms_signals" }
```
Bu dosya sayesinde eklentileri bir lego gibi birbirine bağlayabilecek ve sistemi kod değiştirmeden yeniden kurgulayabileceksiniz.

## 3. Gecikmesiz RAM Transferi (Zero-Copy Shared Memory)
Verileri bir eklentiden diğerine kopyalamak yerine **Paylaşılan Bellek (Shared Memory / Pointers)** mimarisi kullanılacaktır.
* Data Router sistemi, config dosyasını okuduğunda `stream_markprice` isimli bir bellek adresi (örneğin 1MB'lık tahsis edilmiş hafıza bloğu - Pointer) oluşturur.
* Bu hafıza adresinin **Yazma Yetkisini (Write Pointer)** `all_markprices` eklentisine, **Okuma Yetkisini (Read Pointer)** ise `ms_analyzer` eklentisine verir.
* `all_markprices` yeni bir fiyat aldığında bunu belleğe yazar yazmaz, `ms_analyzer` anında (0 milisaniye kopya gecikmesiyle) bu veriyi okuyabilir.

## 4. İletişim Tipleri
1. **Sürekli Akış (Streaming / Pub-Sub):** Yukarıda anlatılan "Paylaşılan Bellek" üzerinden saniyede binlerce kez güncellenen fiyat vb. verilerin aktarımı.
2. **İstek - Cevap (RPC):** Nadir gerçekleşen "Bana geçmiş 10 dakikanın OHLCV verisini ver" gibi doğrudan hedefli talepler. Data Router, bu istekleri de config'de belirtilen rotalara göre hedefe iletip, cevabı anında RAM üzerinden talep edene döndürür.

## 5. Sağlık ve Doğrulama Mekanizması (Health Check & Watchdog)
Veri akışının sağlıklı olup olmadığını denetleyen bağımsız bir denetçi (Watchdog) Data Router içinde çalışacaktır:
* **Heartbeat & Timestamp:** Her paylaşılan bellek bloğunun başında bir `last_updated_timestamp` (Son güncellenme milisaniyesi) bulunur.
* **Sürekli Kontrol:** Data Router her 500ms'de bir tüm stream'leri kontrol eder. Eğer `stream_markprice` 2 saniyedir güncellenmiyorsa (ve piyasa açıkken bunun güncellenmesi gerekiyorsa), sistem orkestratöre ve kullanıcıya "MarkPrice akışı durdu!" uyarısı gönderir.
* **Veri Doğrulaması:** Tanımlı formata uyulup uyulmadığını tespit etmek için header kontrolü (CheckSum / Veri Boyutu) yaparak hatalı (corrupted) bellek yazımlarını anında tespit eder ve hatalı eklentiyi izole eder.

---

## User Review Required

> [!IMPORTANT]
> Bu tasarım, mevcut Orkestratör'den tamamen ayrı, adeta bir "Endüstriyel Veri Veri Yolu (Data Bus / Fabric)" gibi çalışacak yeni bir çekirdek katmanı tanımlar. 
> 
> "Config üzerinden eklentilerin girdi ve çıktılarını birleştiren, gecikmesiz (zero-copy) veri taşıyan ve sağlığı izleyen" bu mimari plan, isteklerinizle tam örtüşüyorsa lütfen onaylayın. Onayladığınız takdirde bu sistemi kodlamaya ve uygulamaya başlayabiliriz.

/* Cycle Orchestrator Architecture & Research Models Dataset */
const docContent = {
    "sections": [
        {
            "id": "overview",
            "title": "Sistem Genel Bakış & Çekirdek Konsept",
            "icon": "⚡",
            "category": "Temel Mimari",
            "summary": "Cycle Orchestrator'ın modüler yapısı, sıfır-gecikmeli paylaşımlı bellek (RAM) mimarisi ve kilitlenmesiz dinamik C-ABI kütüphane yükleme prensipleri.",
            "content": `
                <div class="hero-card">
                    <h1 class="hero-title">Cycle Orchestrator Mimarisi</h1>
                    <p class="hero-desc">
                        Cycle Orchestrator, yüksek frekanslı ticaret (HFT) algoritmaları için geliştirilmiş, ticaret mantığından ve veritabanı işlemlerinden tamamen soyutlanmış, 
                        C-ABI tabanlı sıfır-gecikmeli (zero-copy) paylaşımlı hafıza yönlendirme altyapısıdır.
                    </p>
                    <div class="stats-grid">
                        <div class="stat-item">
                            <div class="stat-value">&lt; 15 ns</div>
                            <div class="stat-label">RAM Bellek Erişim Gecikmesi</div>
                        </div>
                        <div class="stat-item">
                            <div class="stat-value">C-ABI</div>
                            <div class="stat-label">Dinamik Plugin Sözleşmesi</div>
                        </div>
                        <div class="stat-item">
                            <div class="stat-value">Zero-Copy</div>
                            <div class="stat-label">Veri Kopyalamasız Aktarım</div>
                        </div>
                        <div class="stat-item">
                            <div class="stat-value">Lock-Free</div>
                            <div class="stat-label">Eşzamanlı İş Parçacığı Güvenliği</div>
                        </div>
                    </div>
                </div>

                <div class="doc-section">
                    <div class="section-header">
                        <span class="section-icon">🎯</span>
                        <h2 class="section-title">Temel Tasarım İlkeleri</h2>
                    </div>
                    <div class="grid-3">
                        <div class="feature-card">
                            <div class="feature-header">
                                <span class="feature-title">1. Soyutlanmış Çekirdek (Core isolation)</span>
                            </div>
                            <div class="feature-body">
                                Çekirdek sistem içinde sabit kodlanmış ticaret mantığı, ağ istekleri veya SQL sorguları barındırmaz. 
                                Sorumluluğu yalnızca dinamik kütüphaneleri (<code>.so</code> / <code>.dll</code>) yüklemek, yaşam döngüsünü yönetmek ve yönlendirmektir.
                            </div>
                        </div>

                        <div class="feature-card">
                            <div class="feature-header">
                                <span class="feature-title">2. Dinamik Kütüphane Yükleme (libloading)</span>
                            </div>
                            <div class="feature-body">
                                <code>libloading</code> kasası kullanılarak derlenmiş eklentiler çalışma zamanında (runtime) bellek üzerine yüklenir. 
                                Sistem durdurulmadan eklentiler takılıp çıkarılabilir (hot-swappable).
                            </div>
                        </div>

                        <div class="feature-card">
                            <div class="feature-header">
                                <span class="feature-title">3. RAM Tabanlı RPC Protokolü</span>
                            </div>
                            <div class="feature-body">
                                Eklentiler arası iletişim soketler veya IPC mekanizmaları yerine, doğrudan RAM bellek tamponları (<code>Arc&lt;RwLock&lt;Vec&lt;u8&gt;&gt;&gt;</code>) üzerinden C-ABI fonksiyon göstericileri ile gerçekleşir.
                            </div>
                        </div>
                    </div>
                </div>

                <div class="doc-section">
                    <div class="section-header">
                        <span class="section-icon">🏗️</span>
                        <h2 class="section-title">Mimari Katman Hiyerarşisi</h2>
                    </div>
                    <div class="table-container">
                        <table>
                            <thead>
                                <tr>
                                    <th>Katman Adı</th>
                                    <th>Dizin / Kasa</th>
                                    <th>Teknolojiler</th>
                                    <th>Temel Sorumluluk</th>
                                </tr>
                            </thead>
                            <tbody>
                                <tr>
                                    <td><strong>Core Orchestrator</strong></td>
                                    <td><code>crates/core/orchestrator</code></td>
                                    <td>Rust, DashMap, libloading</td>
                                    <td>C-ABI eklenti kaydı, yaşam döngüsü ve bellek pointer yönetimi</td>
                                </tr>
                                <tr>
                                    <td><strong>Flow Engine</strong></td>
                                    <td><code>crates/core/flow_engine</code></td>
                                    <td>Rust, MemoryRouter, AtomicU64</td>
                                    <td>Eklentiler arası DAG veri akışı yönlendirmesi ve son yazılan veri takibi</td>
                                </tr>
                                <tr>
                                    <td><strong>Producers (Plugins)</strong></td>
                                    <td><code>crates/plugins/producers</code></td>
                                    <td>Binance WS, L2 Book</td>
                                    <td>Borsa canlı piyasa verilerini toplayıp paylaşımlı belleğe yazma</td>
                                </tr>
                                <tr>
                                    <td><strong>Analytics (Plugins)</strong></td>
                                    <td><code>crates/plugins/analytics</code></td>
                                    <td>Signal Engine, Breakout</td>
                                    <td>Orderbook ve hacim verilerini anlık işleyip sinyal üretme</td>
                                </tr>
                                <tr>
                                    <td><strong>Execution (Plugins)</strong></td>
                                    <td><code>crates/plugins/execution</code></td>
                                    <td>Binance Trader, C-ABI API</td>
                                    <td>Üretilen sinyalleri riske tabi tutup borsaya iletme veya simülasyon</td>
                                </tr>
                                <tr>
                                    <td><strong>Storage (Plugins)</strong></td>
                                    <td><code>crates/plugins/storage</code></td>
                                    <td>Paper Exchange, SQLite</td>
                                    <td>Kağıt üstü eşleşme motoru ve işlem kayıtlarının tutulması</td>
                                </tr>
                            </tbody>
                        </table>
                    </div>
                </div>
            `
        },
        {
            "id": "market-breakout-model",
            "title": "Market Structure Kırılım & Mikro-Yapı Modeli",
            "icon": "📊",
            "category": "Piyasa Modelleri",
            "summary": "Market structure seviyelerinde kırılım gerçekleşmesinin gerçek zamanlı trade flow, order book, likidite ve liquidation dinamikleriyle matematiksel modellenmesi.",
            "content": `
                <div class="hero-card">
                    <h1 class="hero-title">Market Structure Kırılım & Gerçek Zamanlı Piyasa Dinamikleri Modeli</h1>
                    <p class="hero-desc">
                        Destek ve direnç gibi market structure seviyelerinin kırılımını yalnızca fiyat hareketi üzerinden değerlendirmek yerine, 
                        seviyeye yaklaşma sürecindeki gerçek zamanlı piyasa dinamikleri (trade flow, order-book imbalance, likidite emilimi, fiyat etkisi ve liquidation) ile açıklayan olasılıksal matematiksel model.
                    </p>
                    <div class="stats-grid">
                        <div class="stat-item">
                            <div class="stat-value">P(Breakout|State)</div>
                            <div class="stat-label">Koşullu Gerçekleşme Olasılığı</div>
                        </div>
                        <div class="stat-item">
                            <div class="stat-value">W_{pre}</div>
                            <div class="stat-label">Event-Relative Gözlem Penceresi</div>
                        </div>
                        <div class="stat-item">
                            <div class="stat-value">Likidite & Delta</div>
                            <div class="stat-label">Mikro-Yapı Metrikleri</div>
                        </div>
                        <div class="stat-item">
                            <div class="stat-value">State Machine</div>
                            <div class="stat-label">Durum Bağımlı Pozisyon Yönetimi</div>
                        </div>
                    </div>
                </div>

                <!-- Section 1 & 2: Core Concept & State Lifecycle -->
                <div class="doc-section">
                    <div class="section-header">
                        <span class="section-icon">🔄</span>
                        <h2 class="section-title">1. Temel Hipotez ve Model Yaşam Döngüsü</h2>
                    </div>
                    <p class="hero-desc">
                        Modelin temel varsayımı: <strong>"Market structure gelecekteki fiyat hareketinin potansiyelini tanımlar; market microstructure ise bu potansiyelin gerçekleşme sürecini gösterir."</strong>
                    </p>
                    <div class="feature-card" style="margin-bottom: 20px; text-align: center; background: rgba(0, 243, 255, 0.03); border-color: var(--accent-cyan);">
                        <div style="font-size: 20px; font-weight: 700; color: var(--accent-cyan); font-family: var(--font-mono);">
                            Potential &nbsp;➔&nbsp; Realization &nbsp;➔&nbsp; Sustainability
                        </div>
                    </div>

                    <div class="code-wrapper">
                        <div class="code-header">
                            <span>Model State Flowchart (Yaşam Döngüsü)</span>
                        </div>
                        <pre><code>Market Structure ➔ Level ➔ Potential ➔ Level Activation ➔ Pre-Breakout State
                                                                ↓
                                                           Breakout?
                                                          ↙        ↘
                                                    No (Wait)     Yes (Realization)
                                                                       ↓
                                                                 Sustainability
                                                                       ↓
                                                               Position Decision
                                                                       ↓
                                                                    Outcome ➔ New State</code></pre>
                    </div>
                </div>

                <!-- Section 3 & 4: Activation & Event Window -->
                <div class="doc-section">
                    <div class="section-header">
                        <span class="section-icon">🎯</span>
                        <h2 class="section-title">2. Seviye Belirleme, Aktivasyon (Level Activation) & Event-Relative Pencere</h2>
                    </div>
                    <p class="hero-desc">
                        Fiyat $L$ seviyesinden uzaktayken analiz yapılması gereksizdir. 1 dakikalık $ATR_{1m}$ kullanılarak aktivasyon mesafesi $D$ tanımlanır:
                    </p>

                    <div class="grid-2">
                        <div class="feature-card">
                            <div class="feature-header">
                                <span class="feature-title">Aktivasyon Mesafesi Formülü</span>
                            </div>
                            <div class="feature-body">
                                <div style="font-size: 16px; margin: 12px 0;">
                                    $$D = \\frac{|P - L|}{ATR_{1m}}$$
                                </div>
                                <p>Eğer <strong>$D < k$</strong> ise seviye aktifleşir.</p>
                                <p><strong>Örnek:</strong> $L = 100$, $ATR_{1m} = 0.80$, $k = 0.5$ ise:</p>
                                <p>$$\\text{ActivationDistance} = 0.40 \\implies P_{\\text{activation}} = 99.60$$</p>
                            </div>
                        </div>

                        <div class="feature-card">
                            <div class="feature-header">
                                <span class="feature-title">Event-Relative Pre-Breakout Penceresi</span>
                            </div>
                            <div class="feature-body">
                                <p>Sabit zaman aralıkları (örneğin "son 10 saniye") yerine piyasa olayının kendi süresi kullanılır:</p>
                                <div style="font-size: 16px; margin: 12px 0;">
                                    $$W_{\\text{pre}} = t_{\\text{breakout}} - t_{\\text{activation}}$$
                                </div>
                                <p>Fiyat 99.60'ta olayı başlatıp 100.10'da kırılımı doğrularsa pencere hareketin kendi doğal süresidir (ör. 18 saniye).</p>
                            </div>
                        </div>
                    </div>
                </div>

                <!-- Section 6 - 9: Microstructure Metrics -->
                <div class="doc-section">
                    <div class="section-header">
                        <span class="section-icon">⚡</span>
                        <h2 class="section-title">3. Pre-Breakout Piyasa Dinamikleri (Microstructure Metrics)</h2>
                    </div>
                    <div class="grid-2">
                        <div class="feature-card">
                            <div class="feature-header">
                                <span class="feature-title">Trade Flow & Net Delta</span>
                            </div>
                            <div class="feature-body">
                                <p><strong>Net Delta:</strong></p>
                                <p>$$\\text{Delta} = \\text{BuyVolume} - \\text{SellVolume}$$</p>
                                <p><strong>Delta Oranı:</strong></p>
                                <p>$$\\text{DeltaRatio} = \\frac{\\text{BuyVolume} - \\text{SellVolume}}{\\text{BuyVolume} + \\text{SellVolume}}$$</p>
                            </div>
                        </div>

                        <div class="feature-card">
                            <div class="feature-header">
                                <span class="feature-title">Order Book Likidite Tüketimi</span>
                            </div>
                            <div class="feature-body">
                                <p>Seviyedeki emir defteri likiditesinin tükenme oranı:</p>
                                <div style="font-size: 15px; margin: 10px 0;">
                                    $$\\text{LiquidityDepletion} = \\frac{\\text{InitialLiquidity} - \\text{FinalLiquidity}}{\\text{InitialLiquidity}}$$
                                </div>
                                <p>Yüksek oran, seviyedeki takoz emirlerin emildiğini gösterir.</p>
                            </div>
                        </div>

                        <div class="feature-card">
                            <div class="feature-header">
                                <span class="feature-title">Fiyat Etkisi (Price Impact) & Emilim (Absorption)</span>
                            </div>
                            <div class="feature-body">
                                <p>$$\\text{Impact} = \\frac{\\Delta P / P}{\\text{NormalizedVolume}}$$</p>
                                <ul>
                                    <li><strong>Durum A:</strong> Büyük Alış Hacmi + Yüksek Fiyat Etkisi $\\implies$ Etkili Kırılım.</li>
                                    <li><strong>Durum B:</strong> Büyük Alış Hacmi + Düşük Fiyat Etkisi $\\implies$ <strong>Absorption (Satıcı Emilimi)</strong>.</li>
                                </ul>
                            </div>
                        </div>

                        <div class="feature-card">
                            <div class="feature-header">
                                <span class="feature-title">Liquidation Dynamics & Cascade</span>
                            </div>
                            <div class="feature-body">
                                <p>Perpetual vadeli işlem kaskad geri beslemesi:</p>
                                <p>$$\\text{Price} \\uparrow \\implies \\text{ShortLiquidation} \\uparrow \\implies \\text{ForcedBuy} \\uparrow \\implies \\text{Price} \\uparrow$$</p>
                                <p>Kırılımın normal alıcılar mı yoksa zorunlu tasfiyelerle mi gerçekleştiğini ayırır.</p>
                            </div>
                        </div>
                    </div>
                </div>

                <!-- Section 10 - 13: Probabilities & Position Sizing -->
                <div class="doc-section">
                    <div class="section-header">
                        <span class="section-icon">📈</span>
                        <h2 class="section-title">4. Durum Bazlı Olasılık Tahmini ve Pozisyon Yönetimi</h2>
                    </div>
                    <p class="hero-desc">
                        Sistem deterministik AL/SAT yerine koşullu olasılık modelleri hesaplar:
                    </p>
                    <div class="table-container">
                        <table>
                            <thead>
                                <tr>
                                    <th>Formül / Büyüklük</th>
                                    <th>Matematiksel İfade</th>
                                    <th>Açıklama</th>
                                </tr>
                            </thead>
                            <tbody>
                                <tr>
                                    <td><strong>Durum Vektörü ($S_t$)</strong></td>
                                    <td>$$S_t = \\{\\text{Structure}, \\text{Level}, \\text{Flow}, \\text{OrderBook}, \\text{Liquidity}, \\text{Liquidation}, \\text{Volatility}\\}$$</td>
                                    <td>Tüm mikro yapı değişkenlerinin anlık özet vektörü.</td>
                                </tr>
                                <tr>
                                    <td><strong>Kırılım Olasılığı</strong></td>
                                    <td>$$P(\\text{Breakout} \\mid S_t)$$</td>
                                    <td>Mevcut $S_t$ durumunda $P > L + \\epsilon$ gerçekleşme olasılığı.</td>
                                </tr>
                                <tr>
                                    <td><strong>Sürdürülebilirlik Olasılığı</strong></td>
                                    <td>$$P(\\text{SustainedBreakout} \\mid S_t)$$</td>
                                    <td>Kırılım sonrası akışın devam etme ve retest başarısı olasılığı.</td>
                                </tr>
                                <tr>
                                    <td><strong>Risk Tabanlı Pozisyon Büyüklüğü</strong></td>
                                    <td>$$Q = \\frac{\\text{RiskCapital}}{\\text{InvalidationDistance}}$$</td>
                                    <td>Statik değil, $S_t$ durumuna göre dinamik ölçeklenen büyüklük.</td>
                                </tr>
                            </tbody>
                        </table>
                    </div>
                </div>

                <!-- Section 15 - 17: Research Hypotheses & Experimental Design -->
                <div class="doc-section">
                    <div class="section-header">
                        <span class="section-icon">🧪</span>
                        <h2 class="section-title">5. Araştırma Hipotezleri & Deney Tasarımı</h2>
                    </div>
                    <div class="grid-2">
                        <div class="feature-card">
                            <div class="feature-header">
                                <span class="feature-title">Araştırma Hipotezleri (H1 - H6)</span>
                            </div>
                            <div class="feature-body">
                                <ul>
                                    <li><strong>H1:</strong> Seviyeye yaklaşırken oluşan order-flow imbalance kırılım bilgisini taşır.</li>
                                    <li><strong>H2:</strong> Trade flow ile price impact ilişkisi kırılım kalitesini ayırır.</li>
                                    <li><strong>H3:</strong> Liquidity depletion, kırılımın gerçekleşme olasılığıyla ilişkilidir.</li>
                                    <li><strong>H4:</strong> Liquidation aktivitesi kırılımın yönünü ve sürdürülebilirliğini tahmin eder.</li>
                                    <li><strong>H5:</strong> Event-relative window, sabit zaman pencerelerine göre üstün nitelik sağlar.</li>
                                    <li><strong>H6:</strong> Breakout realization ile Breakout sustainability istatistiksel olarak ayrılabilir.</li>
                                </ul>
                            </div>
                        </div>

                        <div class="feature-card">
                            <div class="feature-header">
                                <span class="feature-title">Deneysel Karşılaştırma Modelleri (Model A - F)</span>
                            </div>
                            <div class="feature-body">
                                <p><strong>Model A:</strong> Market Structure</p>
                                <p><strong>Model B:</strong> Market Structure + OHLCV</p>
                                <p><strong>Model C:</strong> Market Structure + Trade Flow</p>
                                <p><strong>Model D:</strong> Market Structure + Trade Flow + Order Book</p>
                                <p><strong>Model E:</strong> Model D + Liquidation</p>
                                <p><strong>Model F:</strong> Model E + Event-Relative Window</p>
                                <div style="margin-top: 10px; font-weight: 700; color: var(--accent-green);">
                                    Ana Araştırma Sorusu: $$\text{Does } F > A?$$
                                </div>
                            </div>
                        </div>
                    </div>
                </div>

                <!-- Interactive Calculator Tool -->
                <div class="doc-section">
                    <div class="section-header">
                        <span class="section-icon">🧮</span>
                        <h2 class="section-title">6. Interaktif Kırılım Olasılığı & Likidite Hesaplayıcı</h2>
                    </div>
                    <p class="hero-desc">
                        Aşağıdaki canlı hesaplayıcı ile anlık fiyat, ATR, alış/satış hacimleri ve likidite verilerini girerek modelin ürettiği $P(\text{Breakout} \mid S_t)$ ve $P(\text{SustainedBreakout} \mid S_t)$ olasılıklarını simüle edebilirsiniz.
                    </p>

                    <div class="simulator-container">
                        <div class="grid-3" style="margin-bottom: 20px;">
                            <div>
                                <label style="font-size: 12px; color: var(--text-dim);">Fiyat (P):</label>
                                <input type="number" id="calcPrice" value="99.70" step="0.05" oninput="calculateBreakoutModel()" style="width:100%; padding:8px; background:#0e1626; border:1px solid var(--border-color); color:#fff; border-radius:6px;">
                            </div>
                            <div>
                                <label style="font-size: 12px; color: var(--text-dim);">Direnç Seviyesi (L):</label>
                                <input type="number" id="calcLevel" value="100.00" step="0.1" oninput="calculateBreakoutModel()" style="width:100%; padding:8px; background:#0e1626; border:1px solid var(--border-color); color:#fff; border-radius:6px;">
                            </div>
                            <div>
                                <label style="font-size: 12px; color: var(--text-dim);">ATR (1m):</label>
                                <input type="number" id="calcAtr" value="0.80" step="0.05" oninput="calculateBreakoutModel()" style="width:100%; padding:8px; background:#0e1626; border:1px solid var(--border-color); color:#fff; border-radius:6px;">
                            </div>
                            <div>
                                <label style="font-size: 12px; color: var(--text-dim);">Buy Volume (BTC):</label>
                                <input type="number" id="calcBuyVol" value="450" step="10" oninput="calculateBreakoutModel()" style="width:100%; padding:8px; background:#0e1626; border:1px solid var(--border-color); color:#fff; border-radius:6px;">
                            </div>
                            <div>
                                <label style="font-size: 12px; color: var(--text-dim);">Sell Volume (BTC):</label>
                                <input type="number" id="calcSellVol" value="120" step="10" oninput="calculateBreakoutModel()" style="width:100%; padding:8px; background:#0e1626; border:1px solid var(--border-color); color:#fff; border-radius:6px;">
                            </div>
                            <div>
                                <label style="font-size: 12px; color: var(--text-dim);">Initial Ask Depth:</label>
                                <input type="number" id="calcInitLiq" value="1000" step="50" oninput="calculateBreakoutModel()" style="width:100%; padding:8px; background:#0e1626; border:1px solid var(--border-color); color:#fff; border-radius:6px;">
                            </div>
                            <div>
                                <label style="font-size: 12px; color: var(--text-dim);">Current Ask Depth:</label>
                                <input type="number" id="calcCurrLiq" value="250" step="50" oninput="calculateBreakoutModel()" style="width:100%; padding:8px; background:#0e1626; border:1px solid var(--border-color); color:#fff; border-radius:6px;">
                            </div>
                            <div>
                                <label style="font-size: 12px; color: var(--text-dim);">Short Liq Vol ($):</label>
                                <input type="number" id="calcShortLiq" value="350000" step="25000" oninput="calculateBreakoutModel()" style="width:100%; padding:8px; background:#0e1626; border:1px solid var(--border-color); color:#fff; border-radius:6px;">
                            </div>
                        </div>

                        <div class="stats-grid">
                            <div class="stat-item">
                                <div class="stat-value" id="resDistance">0.375</div>
                                <div class="stat-label">Aktivasyon Mesafesi (D)</div>
                            </div>
                            <div class="stat-item">
                                <div class="stat-value" id="resDeltaRatio">+0.579</div>
                                <div class="stat-label">Delta Ratio</div>
                            </div>
                            <div class="stat-item">
                                <div class="stat-value" id="resDepletion">75.0%</div>
                                <div class="stat-label">Liquidity Depletion</div>
                            </div>
                            <div class="stat-item">
                                <div class="stat-value" id="resProbBreakout" style="color: var(--accent-cyan);">84.2%</div>
                                <div class="stat-label">P(Breakout|St)</div>
                            </div>
                            <div class="stat-item">
                                <div class="stat-value" id="resProbSustained" style="color: var(--accent-green);">76.5%</div>
                                <div class="stat-label">P(SustainedBreakout|St)</div>
                            </div>
                        </div>
                    </div>
                </div>
            `
        },
        {
            "id": "core-orchestrator",
            "title": "Core Orchestrator & Sistem Yönetimi",
            "icon": "🧠",
            "category": "Çekirdek Sistem",
            "summary": "orchestrator.rs, system.rs ve endpoint.rs modüllerinin detaylı kod yapısı, dynamic dispatch ve kilitlenmesiz eklenti yönetimi.",
            "content": `
                <div class="doc-section">
                    <div class="section-header">
                        <span class="section-icon">⚙️</span>
                        <h2 class="section-title">Orchestrator Veri Yapısı (orchestrator.rs)</h2>
                    </div>
                    <p class="hero-desc">
                        <code>Orchestrator</code>, sistemdeki yüklenmiş tüm eklenti örneklerini (<code>SystemInstance</code>) bellek seviyesinde tutar.
                        Eşzamanlı okuma işlemlerinin sıfır kilit maliyetiyle yapılabilmesi için <code>Arc&lt;RwLock&lt;Vec&lt;Arc&lt;SystemInstance&gt;&gt;&gt;&gt;</code> yapısını kullanır.
                    </p>

                    <div class="code-wrapper">
                        <div class="code-header">
                            <span>crates/core/orchestrator/src/orchestrator.rs</span>
                            <button class="copy-btn" onclick="copyCode(this)">Kopyala</button>
                        </div>
                        <pre><code><span class="token-keyword">pub struct</span> <span class="token-struct">Orchestrator</span> {
    <span class="token-comment">// Hızlı okuma iterasyonu için RwLock ile korunan vector</span>
    <span class="token-type">systems</span>: <span class="token-type">Arc</span>&lt;<span class="token-type">RwLock</span>&lt;<span class="token-type">Vec</span>&lt;<span class="token-type">Arc</span>&lt;<span class="token-struct">SystemInstance</span>&gt;&gt;&gt;&gt;,
}

<span class="token-keyword">impl</span> <span class="token-struct">Orchestrator</span> {
    <span class="token-keyword">pub fn</span> <span class="token-fn">new</span>() -> <span class="token-type">Self</span> {
        <span class="token-type">Self</span> {
            systems: <span class="token-type">Arc</span>::<span class="token-fn">new</span>(<span class="token-type">RwLock</span>::<span class="token-fn">new</span>(<span class="token-type">Vec</span>::<span class="token-fn">new</span>())),
        }
    }

    <span class="token-comment">// Gecikmesiz, sıfır-kopyalama (zero-copy) endpoint çağrısı</span>
    <span class="token-keyword">#[inline(always)]</span>
    <span class="token-keyword">pub fn</span> <span class="token-fn">call_endpoint</span>(
        &<span class="token-keyword">self</span>, 
        system_id: &<span class="token-type">str</span>, 
        endpoint: <span class="token-struct">StandardEndpoint</span>, 
        payload: &<span class="token-[#e2e8f0]">[u8]</span>, 
        out_buf: &<span class="token-[#e2e8f0]">mut [u8]</span>
    ) -> <span class="token-type">usize</span> {
        <span class="token-keyword">let</span> sys_list = <span class="token-keyword">self</span>.systems.<span class="token-fn">read</span>().<span class="token-fn">unwrap</span>();
        <span class="token-keyword">if let</span> <span class="token-type">Some</span>(sys) = sys_list.<span class="token-fn">iter</span>().<span class="token-fn">find</span>(|s| s.id == system_id || s.name == system_id) {
            <span class="token-keyword">let</span> result = sys.<span class="token-fn">call</span>(endpoint, payload, out_buf);
            
            <span class="token-comment">// Start/Stop durum güncellemelerini atomik olarak yap</span>
            <span class="token-keyword">match</span> endpoint {
                <span class="token-struct">StandardEndpoint</span>::<span class="token-type">Start</span> => {
                    sys.context.is_running.<span class="token-fn">store</span>(<span class="token-keyword">true</span>, core::sync::atomic::<span class="token-type">Ordering</span>::<span class="token-type">Relaxed</span>);
                }
                <span class="token-struct">StandardEndpoint</span>::<span class="token-type">Stop</span> => {
                    sys.context.is_running.<span class="token-fn">store</span>(<span class="token-keyword">false</span>, core::sync::atomic::<span class="token-type">Ordering</span>::<span class="token-type">Relaxed</span>);
                }
                _ => {}
            }
            <span class="token-keyword">if</span> result > <span class="token-num">0</span> {
                sys.context.is_data_valid.<span class="token-fn">store</span>(<span class="token-keyword">true</span>, core::sync::atomic::<span class="token-type">Ordering</span>::<span class="token-type">Relaxed</span>);
            }
            result
        } <span class="token-keyword">else</span> {
            <span class="token-num">0</span>
        }
    }
}</code></pre>
                    </div>
                </div>

                <div class="doc-section">
                    <div class="section-header">
                        <span class="section-icon">📜</span>
                        <h2 class="section-title">Plugin Kontratı (system.rs)</h2>
                    </div>
                    <p class="hero-desc">
                        Her eklenti C-ABI standartlarına uygun ham bir fonksiyon göstericisi (<code>RawEndpointFn</code>) ve dahili durum pointer'ı (<code>*mut c_void</code>) sunar.
                    </p>

                    <div class="code-wrapper">
                        <div class="code-header">
                            <span>crates/core/orchestrator/src/system.rs</span>
                            <button class="copy-btn" onclick="copyCode(this)">Kopyala</button>
                        </div>
                        <pre><code><span class="token-comment">// C-ABI Endpoint fonksiyon imzası (Zero-copy, Sanal Tablo / V-Table Yok)</span>
<span class="token-keyword">pub type</span> <span class="token-struct">RawEndpointFn</span> = <span class="token-keyword">unsafe extern</span> <span class="token-string">"C"</span> <span class="token-keyword">fn</span>(
    plugin_state: *<span class="token-keyword">mut</span> c_void, 
    endpoint_id: <span class="token-type">u32</span>, 
    payload: *<span class="token-keyword">const</span> <span class="token-type">u8</span>, 
    payload_len: <span class="token-type">usize</span>, 
    out_buf: *<span class="token-keyword">mut</span> <span class="token-type">u8</span>, 
    out_max_len: <span class="token-type">usize</span>
) -> <span class="token-type">usize</span>;

<span class="token-keyword">pub struct</span> <span class="token-struct">SystemInstance</span> {
    <span class="token-keyword">pub</span> id: <span class="token-type">String</span>,
    <span class="token-keyword">pub</span> name: <span class="token-type">String</span>,
    <span class="token-keyword">pub</span> context: <span class="token-type">Arc</span>&lt;<span class="token-struct">SystemContext</span>&gt;,
    <span class="token-keyword">pub</span> plugin_state: *<span class="token-keyword">mut</span> c_void,
    <span class="token-keyword">pub</span> endpoint_handler: <span class="token-struct">RawEndpointFn</span>,
}</code></pre>
                    </div>
                </div>
            `
        },
        {
            "id": "flow-engine",
            "title": "Flow Engine & Memory Router",
            "icon": "🔄",
            "category": "Veri Yönlendirme",
            "summary": "Eklentiler arası DAG (Directed Acyclic Graph) veri aktarımı, bellek akışları ve atomik zaman damgası takibi.",
            "content": `
                <div class="doc-section">
                    <div class="section-header">
                        <span class="section-icon">🔀</span>
                        <h2 class="section-title">DAG Veri Yönlendirme Mekanizması</h2>
                    </div>
                    <p class="hero-desc">
                        FlowEngine, üretici (producer) eklentilerden gelen piyasa verilerini <code>MemoryRouter</code> üzerindeki akış başlıklarına (stream_id) yazar.
                        Tüketici (consumer) eklentiler yalnızca yeni veri geldiğinde atomik kontrol (<code>stream.last_updated > last_pushed</code>) ile uyarılır.
                    </p>

                    <div class="grid-2">
                        <div class="feature-card">
                            <div class="feature-header">
                                <span class="feature-title">📥 Pull Stage (Üreticiden Veri Çekme)</span>
                            </div>
                            <div class="feature-body">
                                <code>RawData</code> (Endpoint ID: 5) çağrılarak eklentiden doğrudan RAM tamponuna veri okunur. Eğer JSON akışı çoklu veri barındırıyorsa ilgili stream'lere bölünür.
                            </div>
                        </div>

                        <div class="feature-card">
                            <div class="feature-header">
                                <span class="feature-title">📤 Push Stage (Tüketiciye Veri İletme)</span>
                            </div>
                            <div class="feature-body">
                                Eklentinin abone olduğu akışta güncelleme varsa, ilk 32 byte stream_id başlığı olmak üzere payload hazırlanır ve eklentinin <code>Inbox</code> (Endpoint ID: 6) fonksiyonuna iletilir.
                            </div>
                        </div>
                    </div>

                    <div class="code-wrapper">
                        <div class="code-header">
                            <span>crates/core/flow_engine/src/engine.rs</span>
                            <button class="copy-btn" onclick="copyCode(this)">Kopyala</button>
                        </div>
                        <pre><code><span class="token-comment">// FlowEngine ana döngüsü (Run Loop)</span>
<span class="token-keyword">pub fn</span> <span class="token-fn">run_loop</span>&lt;<span class="token-type">F</span>&gt;(&<span class="token-keyword">self</span>, <span class="token-keyword">mut</span> caller: <span class="token-type">F</span>)
<span class="token-keyword">where</span>
    <span class="token-type">F</span>: <span class="token-type">FnMut</span>(&<span class="token-[#e2e8f0]">str</span>, <span class="token-[#e2e8f0]">u32</span>, &<span class="token-[#e2e8f0]">[u8]</span>, &<span class="token-[#e2e8f0]">mut [u8]</span>) -> <span class="token-type">usize</span>,
{
    <span class="token-[#e2e8f0]">...</span>
    <span class="token-comment">// Outbox mesajlarını oku ve hedef eklentinin Inbox'ına teslim et</span>
    <span class="token-keyword">let</span> bytes_read = <span class="token-fn">caller</span>(&plugin.plugin_name, <span class="token-num">7</span>, &[], &<span class="token-keyword">mut</span> temp_buf); <span class="token-comment">// 7 = Outbox</span>
    <span class="token-keyword">if</span> bytes_read > <span class="token-num">0</span> {
        <span class="token-keyword">if let</span> <span class="token-[#e2e8f0]">Ok</span>(messages) = serde_json::<span class="token-fn">from_slice</span>(&temp_buf[..bytes_read]) {
            <span class="token-keyword">if let</span> <span class="token-[#e2e8f0]">Some</span>(arr) = messages.<span class="token-fn">as_array</span>() {
                <span class="token-keyword">for</span> msg <span class="token-keyword">in</span> arr {
                    <span class="token-keyword">if let</span> <span class="token-[#e2e8f0]">Some</span>(target) = msg.<span class="token-fn">get</span>(<span class="token-string">"to"</span>).<span class="token-fn">and_then</span>(|t| t.<span class="token-fn">as_str</span>()) {
                        <span class="token-[#e2e8f0]">...</span>
                        <span class="token-fn">caller</span>(target, <span class="token-num">6</span>, &payload_bytes, &<span class="token-keyword">mut</span> temp_buf); <span class="token-comment">// 6 = Inbox</span>
                    }
                }
            }
        }
    }
}</code></pre>
                    </div>
                </div>
            `
        },
        {
            "id": "c-abi-spec",
            "title": "C-ABI & Standart Endpoint Özellikleri",
            "icon": "📐",
            "category": "Protokol & Şema",
            "summary": "StandardEndpoint enum sabitleri, byte hizalaması (alignment) ve C-ABI RPC çağrı düzeni.",
            "content": `
                <div class="doc-section">
                    <div class="section-header">
                        <span class="section-icon">🔌</span>
                        <h2 class="section-title">StandardEndpoint Numaralandırması (endpoint.rs)</h2>
                    </div>
                    
                    <div class="table-container">
                        <table>
                            <thead>
                                <tr>
                                    <th>ID (u32)</th>
                                    <th>Endpoint Adı</th>
                                    <th>Yön</th>
                                    <th>Açıklama / PayLoad Yapısı</th>
                                </tr>
                            </thead>
                            <tbody>
                                <tr>
                                    <td><code>0</code></td>
                                    <td><code>Start</code></td>
                                    <td>Orchestrator ➔ Plugin</td>
                                    <td>Eklentinin çalışmasını ve dinleme döngülerini başlatır.</td>
                                </tr>
                                <tr>
                                    <td><code>1</code></td>
                                    <td><code>Stop</code></td>
                                    <td>Orchestrator ➔ Plugin</td>
                                    <td>Eklentiyi durdurur ve kaynakları serbest bırakır.</td>
                                </tr>
                                <tr>
                                    <td><code>2</code></td>
                                    <td><code>IsWorking</code></td>
                                    <td>Orchestrator ➔ Plugin</td>
                                    <td>Eklentinin aktif olup olmadığını sorgular (1=Aktif, 0=Pasif).</td>
                                </tr>
                                <tr>
                                    <td><code>3</code></td>
                                    <td><code>DataValid</code></td>
                                    <td>Orchestrator ➔ Plugin</td>
                                    <td>Son üretilen bellek tamponunun geçerli olup olmadığını doğrular.</td>
                                </tr>
                                <tr>
                                    <td><code>4</code></td>
                                    <td><code>DataMonitor</code></td>
                                    <td>Orchestrator ➔ Plugin</td>
                                    <td>TUI/Web arayüzü için 1MB kadar ham durum çıktısı verir.</td>
                                </tr>
                                <tr>
                                    <td><code>5</code></td>
                                    <td><code>RawData</code></td>
                                    <td>Orchestrator ➔ Plugin</td>
                                    <td>Üretici eklentinin ürettiği son piyasa paketi çıktısı.</td>
                                </tr>
                                <tr>
                                    <td><code>6</code></td>
                                    <td><code>Inbox</code></td>
                                    <td>Engine ➔ Plugin</td>
                                    <td>Gelen veri akışı. İlk 32 byte <code>stream_id</code>, kalan byte'lar binary payload.</td>
                                </tr>
                                <tr>
                                    <td><code>7</code></td>
                                    <td><code>Outbox</code></td>
                                    <td>Plugin ➔ Engine</td>
                                    <td>Eklentinin diğer eklentilere göndermek istediği JSON/Binary mesaj listesi.</td>
                                </tr>
                                <tr>
                                    <td><code>8</code></td>
                                    <td><code>GetSubscriptions</code></td>
                                    <td>Orchestrator ➔ Plugin</td>
                                    <td>Eklentinin dinlediği akış başlıklarının listesini döndürür.</td>
                                </tr>
                            </tbody>
                        </table>
                    </div>
                </div>

                <div class="doc-section">
                    <div class="section-header">
                        <span class="section-icon">💾</span>
                        <h2 class="section-title">Bellek Hizalaması & Bellek Formatı (Memory Layout)</h2>
                    </div>
                    <div class="grid-2">
                        <div class="feature-card">
                            <div class="feature-header">
                                <span class="feature-title">Inbox Bellek Yapısı (Stream Header)</span>
                            </div>
                            <div class="feature-body">
                                <p><strong>Byte 0 - 31:</strong> ASCII Stream ID (Sağ taraf 0x00 null-padding)</p>
                                <p><strong>Byte 32+:</strong> JSON veya Binary Veri Bloğu (Orderbook, Trade, Signal)</p>
                            </div>
                        </div>

                        <div class="feature-card">
                            <div class="feature-header">
                                <span class="feature-title">Sıfır-Kopyalama Garantisi</span>
                            </div>
                            <div class="feature-body">
                                Veri işaretçileri (<code>*const u8</code>) doğrudan işletim sistemi RAM adresi üzerinden aktarılır. 
                                Rust tarafında heap reallocation yapılmaz.
                            </div>
                        </div>
                    </div>
                </div>
            `
        },
        {
            "id": "plugins",
            "title": "Eklenti Ekosistemi & HFT Bileşenleri",
            "icon": "🧩",
            "category": "Eklentiler",
            "summary": "binance_gateway, plugin_breakout, binance_trader ve plugin_paper_exchange modüllerinin görev ve akış detayları.",
            "content": `
                <div class="doc-section">
                    <div class="section-header">
                        <span class="section-icon">🏭</span>
                        <h2 class="section-title">Mevcut Eklenti Listesi</h2>
                    </div>
                    <div class="grid-2">
                        <div class="feature-card">
                            <div class="feature-header">
                                <span class="feature-title">1. Binance Gateway (Producer)</span>
                            </div>
                            <div class="feature-body">
                                <p><strong>Dizin:</strong> <code>crates/plugins/producers/binance_gateway</code></p>
                                <p>Binance WebSocket L2 derinlik ve son işlemler akışına bağlanır. Veriyi süzüp paylaşımlı belleğe push eder.</p>
                            </div>
                        </div>

                        <div class="feature-card">
                            <div class="feature-header">
                                <span class="feature-title">2. Plugin Breakout (Analytics)</span>
                            </div>
                            <div class="feature-body">
                                <p><strong>Dizin:</strong> <code>crates/plugins/analytics/plugin_breakout</code></p>
                                <p>Orderbook dengesizliklerini (imbalance) ve ani hacim kırılımlarını mikro-saniyeler içinde hesaplayarak AL/SAT sinyali üretir.</p>
                            </div>
                        </div>

                        <div class="feature-card">
                            <div class="feature-header">
                                <span class="feature-title">3. Binance Trader (Execution)</span>
                            </div>
                            <div class="feature-body">
                                <p><strong>Dizin:</strong> <code>crates/plugins/execution/binance_trader</code></p>
                                <p>Gelen sinyalleri risk kontrolünden geçirip Binance REST API üzerinden gerçek emirleri iletir.</p>
                            </div>
                        </div>

                        <div class="feature-card">
                            <div class="feature-header">
                                <span class="feature-title">4. Paper Exchange (Storage & Engine)</span>
                            </div>
                            <div class="feature-body">
                                <p><strong>Dizin:</strong> <code>crates/plugins/storage/plugin_paper_exchange</code></p>
                                <p>Gerçek borsa yerine geçen in-memory simülasyon eşleşme motoru. İşlem geçmişini SQLite (<code>paper_exchange.db</code>) üzerine yazar.</p>
                            </div>
                        </div>
                    </div>
                </div>
            `
        },
        {
            "id": "dag-simulator",
            "title": "Interaktif DAG & Paylaşımlı Bellek Simülatörü",
            "icon": "🗺️",
            "category": "Interaktif Araçlar",
            "summary": "Cycle Orchestrator eklentilerinin bellek üzerindeki anlık veri hareketlerini görselleştiren interaktif SVG kanvası.",
            "content": `
                <div class="doc-section">
                    <div class="section-header">
                        <span class="section-icon">⚡</span>
                        <h2 class="section-title">Canlı Veri Akış Simülatörü</h2>
                    </div>
                    <p class="hero-desc">
                        Aşağıdaki simülatör, Binance Gateway'den alınan piyasa verisinin <code>MemoryRouter</code> tamponları üzerinden 
                        Breakout Analytics ve Paper Exchange eklentilerine sıfır gecikmeyle nasıl aktarıldığını temsil eder.
                    </p>

                    <div class="simulator-container">
                        <div class="sim-controls">
                            <button class="sim-btn" id="startSimBtn" onclick="toggleSimulation()">▶ Simülasyonu Başlat</button>
                            <button class="sim-btn secondary" onclick="triggerPacket()">⚡ Anlık Paket Gönder</button>
                            <span style="font-size: 13px; color: var(--text-muted); margin-left: auto;">
                                Akış Durumu: <strong id="simStatusText" style="color: var(--accent-green);">IDLE</strong>
                            </span>
                        </div>
                        
                        <div class="canvas-wrapper">
                            <svg id="dagCanvas"></svg>
                        </div>

                        <div class="canvas-legend">
                            <div class="legend-item">
                                <div class="legend-color" style="background: var(--accent-cyan);"></div>
                                <span>Producer Node</span>
                            </div>
                            <div class="legend-item">
                                <div class="legend-color" style="background: var(--accent-purple);"></div>
                                <span>Shared RAM Router</span>
                            </div>
                            <div class="legend-item">
                                <div class="legend-color" style="background: var(--accent-green);"></div>
                                <span>Analytics & Execution</span>
                            </div>
                        </div>
                    </div>
                </div>
            `
        }
    ]
};

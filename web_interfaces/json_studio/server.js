const express = require('express');
const cors = require('cors');
const fs = require('fs');
const path = require('path');

const app = express();
const PORT = process.env.PORT || 3030;

let CONFIG_FILE = path.resolve(__dirname, '../../flow_config.json');
if (!fs.existsSync(CONFIG_FILE)) {
  CONFIG_FILE = path.resolve(__dirname, '../flow_config.json');
}
if (!fs.existsSync(CONFIG_FILE)) {
  CONFIG_FILE = path.resolve(process.cwd(), 'flow_config.json');
}

app.use(cors());
app.use(express.json({ limit: '10mb' }));
app.use(express.static(path.join(__dirname, 'public')));

// Standard plugin template definitions available in cycle-orc
const KNOWN_PLUGINS = [
  {
    name: 'plugin_binance_gateway',
    category: 'Producer / Gateway',
    description: 'Binance Futures WebSocket canlı veri akışı (MarkPrice, Trades, AggTrades, Depth, Liquidations)',
    default_outputs: ['stream_markprice', 'stream_trades', 'stream_aggtrades', 'stream_depth', 'stream_bestprice', 'stream_liquidations'],
    default_params: { symbols: ['BTCUSDT', 'ETHUSDT'] }
  },
  {
    name: 'plugin_ohlcv_fetcher',
    category: 'Producer / Data Fetcher',
    description: 'Binance REST API üzerinden mum (OHLCV) verisi çekici',
    default_outputs: ['btc_15m', 'eth_5m'],
    default_params: {}
  },
  {
    name: 'plugin_ms_analyzer',
    category: 'Processor / Strategy',
    description: 'Çoklu zaman dilimi (Multi-Structure) teknik analiz motoru',
    default_outputs: ['ms_signals_btc', 'ms_signals_eth'],
    default_params: { analysis_mode: 'deep' }
  },
  {
    name: 'plugin_breakout',
    category: 'Processor / Strategy',
    description: 'Kırılım ve Volatilité tespit stratejisi',
    default_outputs: ['breakout_signals'],
    default_params: { threshold: 0.02 }
  },
  {
    name: 'plugin_paper_exchange',
    category: 'Execution / Exchange',
    description: 'Sanal borsa simülatörü (Paper Trading)',
    default_outputs: ['paper_exchange_updates'],
    default_params: {}
  },
  {
    name: 'plugin_scout',
    category: 'Monitoring',
    description: 'Piyasa tarayıcısı ve fırsat bulucu',
    default_outputs: ['scout_alerts'],
    default_params: {}
  },
  {
    name: 'plugin_binance_sqlite',
    category: 'Storage',
    description: 'Piyasa akışlarını SQLite veritabanına kaydeder',
    default_outputs: [],
    default_params: { db_path: 'binance_market_data.db' }
  },
  {
    name: 'plugin_sqlite_query',
    category: 'Storage / Query',
    description: 'SQLite veritabanı sorgulayıcı',
    default_outputs: [],
    default_params: { db_path: 'binance_market_data.db' }
  },
  {
    name: 'plugin_aggtrade_stats',
    category: 'Analytics',
    description: 'Aggregated trade istatistikleri ve hacim analizi',
    default_outputs: [],
    default_params: { window_ms: 60000 }
  }
];

// GET /api/status
app.get('/api/status', (req, res) => {
  const exists = fs.existsSync(CONFIG_FILE);
  let stats = null;
  if (exists) {
    stats = fs.statSync(CONFIG_FILE);
  }
  res.json({
    status: 'online',
    config_file: CONFIG_FILE,
    file_exists: exists,
    last_modified: stats ? stats.mtime : null,
    size_bytes: stats ? stats.size : 0
  });
});

// GET /api/config
app.get('/api/config', (req, res) => {
  try {
    if (!fs.existsSync(CONFIG_FILE)) {
      return res.status(404).json({ error: 'flow_config.json dosyası bulunamadı.' });
    }
    const rawText = fs.readFileSync(CONFIG_FILE, 'utf-8');
    const jsonParsed = JSON.parse(rawText);
    res.json({
      success: true,
      raw: rawText,
      data: jsonParsed,
      filepath: CONFIG_FILE
    });
  } catch (err) {
    res.status(500).json({ error: 'Config okuma veya JSON ayrıştırma hatası: ' + err.message });
  }
});

// POST /api/config
app.post('/api/config', (req, res) => {
  try {
    const { data, raw } = req.body;
    let contentToWrite = '';

    if (raw !== undefined && typeof raw === 'string') {
      // Validate JSON formatting first
      JSON.parse(raw);
      contentToWrite = raw;
    } else if (data !== undefined) {
      contentToWrite = JSON.stringify(data, null, 2);
    } else {
      return res.status(400).json({ error: 'Yazılacak veri ("data" veya "raw") sağlanmadı.' });
    }

    // Create backup file first if original exists
    if (fs.existsSync(CONFIG_FILE)) {
      const backupPath = `${CONFIG_FILE}.bak`;
      fs.copyFileSync(CONFIG_FILE, backupPath);
    }

    fs.writeFileSync(CONFIG_FILE, contentToWrite, 'utf-8');

    res.json({
      success: true,
      message: 'flow_config.json başarıyla güncellendi ve yedeklendi.',
      filepath: CONFIG_FILE
    });
  } catch (err) {
    res.status(400).json({ error: 'Kaydetme hatası (Geçersiz JSON formatı): ' + err.message });
  }
});

// GET /api/plugins
app.get('/api/plugins', (req, res) => {
  res.json({
    known_plugins: KNOWN_PLUGINS
  });
});

// POST /api/validate
app.post('/api/validate', (req, res) => {
  try {
    const { data } = req.body;
    const issues = [];
    const producedStreams = new Map(); // stream_id -> plugin_name

    if (!Array.isArray(data)) {
      return res.json({
        valid: false,
        issues: [{ severity: 'error', message: 'flow_config.json bir dizi (Array) olmalıdır.' }]
      });
    }

    // Pass 1: Collect outputs
    data.forEach((plugin, pIdx) => {
      const pName = plugin.plugin_name || `Plugin #${pIdx + 1}`;
      if (Array.isArray(plugin.plugin_outputs)) {
        plugin.plugin_outputs.forEach((outStream) => {
          producedStreams.set(outStream, pName);
        });
      }
    });

    // Pass 2: Validate inputs & params
    data.forEach((plugin, pIdx) => {
      const pName = plugin.plugin_name || `Plugin #${pIdx + 1}`;
      if (!plugin.plugin_name) {
        issues.push({ severity: 'error', plugin: pName, message: `Plugin #${pIdx + 1} 'plugin_name' alanına sahip değil.` });
      }

      if (Array.isArray(plugin.plugin_inputs)) {
        plugin.plugin_inputs.forEach((inp, iIdx) => {
          if (!inp.source) {
            issues.push({ severity: 'warning', plugin: pName, message: `Girdi #${iIdx + 1} için 'source' plugin adı belirtilmemiş.` });
          }
          if (!inp.stream_id) {
            issues.push({ severity: 'error', plugin: pName, message: `Girdi #${iIdx + 1} için 'stream_id' belirtilmemiş.` });
          } else if (!producedStreams.has(inp.stream_id)) {
            issues.push({
              severity: 'warning',
              plugin: pName,
              message: `'${inp.stream_id}' isimli akış kaynağı sistemdeki hiçbir eklentinin çıktısında üretilmiyor (Askıda akış).`
            });
          }
        });
      }
    });

    res.json({
      valid: issues.filter(i => i.severity === 'error').length === 0,
      issues: issues,
      produced_streams: Array.from(producedStreams.entries()).map(([stream, producer]) => ({ stream, producer }))
    });
  } catch (err) {
    res.status(400).json({ valid: false, issues: [{ severity: 'error', message: 'Doğrulama hatası: ' + err.message }] });
  }
});

app.listen(PORT, () => {
  console.log(`=======================================================`);
  console.log(` 🚀 Cycle-ORC Görsel JSON & Akış Editörü Başlatıldı!`);
  console.log(` 📍 Yerel Web Arayüzü: http://localhost:${PORT}`);
  console.log(` 📄 Hedef Konfigürasyon: ${CONFIG_FILE}`);
  console.log(`=======================================================`);
});

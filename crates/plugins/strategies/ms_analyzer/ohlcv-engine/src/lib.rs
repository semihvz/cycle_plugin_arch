// ============================================================================
// ohlcv-engine (yerel, bağımsız sürüm)
// ============================================================================
// detect-ms'de kullanılan Kline veri modeli ve Binance veri istemcisi.
// Binance public API üzerinden OHLCV verisi çeker (api.binance.com + yedek
// data-api.binance.vision). Dış bağımlılık yoktur; bu klasörün içindedir.
// ============================================================================

pub mod client;

use rust_decimal::Decimal;

/// Tek bir OHLCV mumu (Binance klines formatı)
#[derive(Debug, Clone)]
pub struct Kline {
    pub open_time: u64,
    pub close_time: u64,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: Decimal,
    /// Aggressor alıcı hacmi (delta hesabı için)
    pub taker_buy_base_asset_volume: Decimal,
}

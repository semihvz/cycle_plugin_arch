pub const BINANCE_REST: &str = "https://fapi.binance.com";
pub const BINANCE_WS: &str = "wss://fstream.binance.com";

pub const ANALYSIS_INTERVAL_SECS: u64 = 1;
pub const WINDOW_SECONDS: f64 = 3.0;

pub const BOOK_TICKER_CHUNK_SIZE: usize = 180;
pub const DEPTH_STREAM_CHUNK_SIZE: usize = 30;
pub const DEPTH_CANDIDATE_COUNT: usize = 60;
pub const DEPTH_REBALANCE_SECS: f64 = 2.0;
pub const DEPTH_LEVELS: usize = 10;
pub const DEPTH_UPDATE_SPEED: &str = "100ms";

pub const MIN_SPREAD_BPS: f64 = 0.25;
pub const MIN_TICKS_PER_SECOND: f64 = 0.20;
pub const STALE_SYMBOL_SECS: f64 = 1.5;

pub const WS_HEARTBEAT_SECS: u64 = 20;
pub const WS_BACKOFF_BASE_SECS: f64 = 0.75;
pub const WS_BACKOFF_CAP_SECS: f64 = 10.0;

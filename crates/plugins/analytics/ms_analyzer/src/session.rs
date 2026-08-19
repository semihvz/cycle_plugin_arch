// ============================================================================
// MSMP 2.0 — KATMAN 1: ZAMAN PENCERESİ (Session-Based & Ağırlıklı)
// ============================================================================
// Sabit mum sayısı yerine Aktif İşlem Seansları (UTC 08:00-16:00) kullanılır.
// 3 pencere: Core (%40), Amplified (%30), Acute (%30)
// ============================================================================

use ohlcv_engine::Kline;
use rust_decimal::Decimal;
use rust_decimal::prelude::*;

/// Seans bazlı zaman pencereleri
#[derive(Debug, Clone, Copy)]
pub enum SessionWindow {
    /// Son 5 İşlem Günü (120 Aktif Saat) — Ağırlık: %40
    Core,
    /// Son 20 İşlem Günü (480 Aktif Saat) — Ağırlık: %30
    Amplified,
    /// Son 24 Aktif Saat (Bugünkü Seans) — Ağırlık: %30
    Acute,
}

impl SessionWindow {
    /// Pencere ağırlık katsayısı
    pub fn weight(&self) -> Decimal {
        match self {
            SessionWindow::Core => Decimal::from_str("0.40").unwrap(),
            SessionWindow::Amplified => Decimal::from_str("0.30").unwrap(),
            SessionWindow::Acute => Decimal::from_str("0.30").unwrap(),
        }
    }

    /// Penceredeki aktif saat sayısı
    pub fn active_hours(&self) -> u64 {
        match self {
            SessionWindow::Core => 120,
            SessionWindow::Amplified => 480,
            SessionWindow::Acute => 24,
        }
    }
}

/// UTC saatini milisaniye timestamp'ten çıkarır
fn utc_hour_from_timestamp(ts_ms: u64) -> u64 {
    (ts_ms / 3_600_000) % 24
}

/// Londra + NY seansı aktif mi? (UTC 08:00 – 16:00)
pub fn is_active_session(ts_ms: u64) -> bool {
    let hour = utc_hour_from_timestamp(ts_ms);
    hour >= 8 && hour < 16
}

/// Seans ağırlığı: Aktif seans mumlarına 1.0, dışına 0.5
pub fn session_weight(ts_ms: u64) -> Decimal {
    if is_active_session(ts_ms) {
        Decimal::ONE
    } else {
        Decimal::from_str("0.5").unwrap()
    }
}

/// Kline'ları pencereye göre filtreler (zaman aralığına göre)
pub fn filter_by_window<'a>(klines: &'a [Kline], window: SessionWindow) -> Vec<&'a Kline> {
    if klines.is_empty() {
        return vec![];
    }
    let latest_time = klines.last().unwrap().close_time;
    let window_ms = window.active_hours() * 3_600_000;

    klines
        .iter()
        .filter(|k| latest_time.saturating_sub(k.open_time) <= window_ms)
        .collect()
}

/// 3 pencereden gelen skorları Ağırlıklı Ortalama ile birleştirir.
/// Hiçbir pencere diğerini ezmez; matematiksel üstünlük sağlanır.
pub fn weighted_merge(core_score: Decimal, amp_score: Decimal, acute_score: Decimal) -> Decimal {
    core_score * SessionWindow::Core.weight()
        + amp_score * SessionWindow::Amplified.weight()
        + acute_score * SessionWindow::Acute.weight()
}

/// Confluence Index: 3 pencerenin trend yönü uyum yüzdesi
pub fn confluence_index(core_score: Decimal, amp_score: Decimal, acute_score: Decimal) -> Decimal {
    let scores = [core_score, amp_score, acute_score];
    let positive_count = scores.iter().filter(|&&d| d > Decimal::ZERO).count();
    let negative_count = scores.iter().filter(|&&d| d < Decimal::ZERO).count();

    let dominant_count = positive_count.max(negative_count);
    (Decimal::from(dominant_count) / Decimal::from(3)) * Decimal::ONE_HUNDRED
}

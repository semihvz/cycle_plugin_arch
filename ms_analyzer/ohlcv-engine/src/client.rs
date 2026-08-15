// ============================================================================
// BinanceClient — OHLCV veri istemcisi
// ============================================================================

use std::error::Error;

use rust_decimal::prelude::*;
use rust_decimal::Decimal;

use crate::Kline;

pub struct BinanceClient {
    http: reqwest::Client,
}

impl BinanceClient {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::builder()
                .build()
                .expect("BinanceClient HTTP istemcisi kurulamadı"),
        }
    }

    /// Klines verisini çeker. İlk başarılı host üzerinden döner.
    /// api.binance.com başarısız olursa data-api.binance.vision denenir.
    pub async fn fetch_klines(
        &self,
        symbol: &str,
        interval: &str,
        limit: usize,
    ) -> Result<Vec<Kline>, Box<dyn Error + Send + Sync>> {
        let mut last_err: Option<Box<dyn Error + Send + Sync>> = None;

        // Spot host'lar, ardından USDT-M futures (fapi)
        for (base, path) in [
            ("https://api.binance.com", "/api/v3/klines"),
            ("https://data-api.binance.vision", "/api/v3/klines"),
            ("https://fapi.binance.com", "/fapi/v1/klines"),
        ] {
            let url = format!("{base}{path}?symbol={symbol}&interval={interval}&limit={limit}");

            match self.http.get(&url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    return self.parse_response(resp.json().await?);
                }
                Ok(resp) => {
                    last_err = Some(format!("HTTP {} — {}", resp.status(), url).into());
                }
                Err(e) => {
                    last_err = Some(Box::new(e));
                }
            }
        }

        Err(last_err.unwrap_or_else(|| "Binance veri alınamadı".into()))
    }

    fn parse_response(
        &self,
        rows: serde_json::Value,
    ) -> Result<Vec<Kline>, Box<dyn Error + Send + Sync>> {
        let rows = rows
            .as_array()
            .ok_or_else(|| "Binance yanıtı dizi değil".to_string())?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let cells = row.as_array().ok_or_else(|| "Kline satırı geçersiz".to_string())?;
            if cells.len() < 12 {
                continue;
            }

            let cell = |i: usize| {
                cells[i]
                    .as_str()
                    .map(|s| Decimal::from_str(s))
                    .transpose()
                    .ok()
                    .flatten()
                    .unwrap_or(Decimal::ZERO)
            };

            out.push(Kline {
                open_time: cells[0].as_u64().unwrap_or(0),
                open: cell(1),
                high: cell(2),
                low: cell(3),
                close: cell(4),
                volume: cell(5),
                close_time: cells[6].as_u64().unwrap_or(0),
                taker_buy_base_asset_volume: cell(9),
            });
        }

        Ok(out)
    }
}

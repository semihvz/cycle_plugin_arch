use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use rand::Rng;
use serde_json::Value;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use crate::config;
use crate::utils::parse_json;

pub type Handler = Box<dyn FnMut(Value) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send>;

pub struct BinanceClient {
    http: reqwest::Client,
}

impl BinanceClient {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .pool_max_idle_per_host(8)
            .build()
            .expect("reqwest client build failed");
        Self { http }
    }

    pub async fn fetch_symbols(&self) -> Result<Vec<String>, reqwest::Error> {
        let url = format!("{}/fapi/v1/exchangeInfo", config::BINANCE_REST);
        let data: Value = self.http.get(&url).send().await?.json().await?;

        let mut symbols: Vec<String> = data["symbols"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter(|sym| {
                        sym["symbol"].as_str().map_or(false, |s| s.ends_with("USDT"))
                            && sym["status"].as_str() == Some("TRADING")
                            && sym["contractType"].as_str() == Some("PERPETUAL")
                    })
                    .filter_map(|sym| sym["symbol"].as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        symbols.sort();
        Ok(symbols)
    }

    pub async fn stream_book_tickers(&self, symbols: &[String], handler: Handler) {
        let streams: Vec<String> = symbols
            .iter()
            .map(|s| format!("{}@bookTicker", s.to_lowercase()))
            .collect();
        let url = format!("{}/stream?streams={}", config::BINANCE_WS, streams.join("/"));
        self.stream_loop(url, "bookTicker", handler).await;
    }

    pub async fn stream_partial_depths(&self, symbols: &[String], handler: Handler) {
        let suffix = format!("depth{}@{}", config::DEPTH_LEVELS, config::DEPTH_UPDATE_SPEED);
        let streams: Vec<String> = symbols
            .iter()
            .map(|s| format!("{}@{}", s.to_lowercase(), suffix))
            .collect();
        let url = format!("{}/stream?streams={}", config::BINANCE_WS, streams.join("/"));
        self.stream_loop(url, "partialDepth", handler).await;
    }

    async fn stream_loop(&self, url: String, stream_name: &'static str, mut handler: Handler) {
        let mut backoff = config::WS_BACKOFF_BASE_SECS;

        loop {
            match connect_async(&url).await {
                Ok((mut ws, _)) => {
                    backoff = config::WS_BACKOFF_BASE_SECS;
                    let mut heartbeat = tokio::time::interval(Duration::from_secs(config::WS_HEARTBEAT_SECS));
                    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

                    loop {
                        tokio::select! {
                            _ = heartbeat.tick() => {
                                if ws.send(Message::Ping(Vec::new().into())).await.is_err() {
                                    break;
                                }
                            }
                            msg = ws.next() => {
                                match msg {
                                    Some(Ok(Message::Text(text))) => {
                                        let payload = parse_json(&text);
                                        let data = payload.get("data").cloned().unwrap_or(payload);
                                        handler(data).await;
                                    }
                                    Some(Ok(Message::Ping(p))) => {
                                        if ws.send(Message::Pong(p)).await.is_err() {
                                            break;
                                        }
                                    }
                                    Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                                    _ => {}
                                }
                            }
                        }
                    }
                }
                Err(err) => {
                    tracing::error!("{} stream hatasi: {}", stream_name, err);
                }
            }

            let jitter: f64 = rand::thread_rng().gen_range(0.0..0.5);
            let sleep_for = (backoff + jitter).min(config::WS_BACKOFF_CAP_SECS);
            tokio::time::sleep(Duration::from_secs_f64(sleep_for)).await;
            backoff = (backoff * 2.0).min(config::WS_BACKOFF_CAP_SECS);
        }
    }
}

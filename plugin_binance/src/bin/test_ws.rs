use futures_util::StreamExt;
use tokio_tungstenite::connect_async;
use url::Url;

#[tokio::main]
async fn main() {
    println!("Connecting to fstream.binance.com/ws/btcusdt@ticker ...");
    let connect_url = Url::parse("wss://fstream.binance.com/ws/btcusdt@ticker").unwrap();
    if let Ok((mut ws_stream, _)) = connect_async(connect_url).await {
        for _ in 0..3 {
            if let Some(msg) = ws_stream.next().await {
                println!("Msg: {:?}", msg);
            }
        }
    }
}

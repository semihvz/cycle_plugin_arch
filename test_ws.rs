use tokio_tungstenite::connect_async;
use futures_util::StreamExt;

#[tokio::main]
async fn main() {
    let url = "wss://fstream.binance.com/stream?streams=btcusdt@markPrice@1s";
    let url2 = "wss://fstream.binance.com/market/stream?streams=btcusdt@markPrice@1s";
    let url3 = "wss://fstream.binance.com/public/stream?streams=btcusdt@markPrice@1s";
    
    println!("Trying url1: {}", url);
    match connect_async(url).await {
        Ok(_) => println!("url1 SUCCESS"),
        Err(e) => println!("url1 ERROR: {}", e),
    }

    println!("Trying url2: {}", url2);
    match connect_async(url2).await {
        Ok(_) => println!("url2 SUCCESS"),
        Err(e) => println!("url2 ERROR: {}", e),
    }

    println!("Trying url3: {}", url3);
    match connect_async(url3).await {
        Ok(_) => println!("url3 SUCCESS"),
        Err(e) => println!("url3 ERROR: {}", e),
    }
}

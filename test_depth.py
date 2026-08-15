import asyncio
import websockets
import json
import sys

async def test_depth(url):
    print(f"Connecting to {url}")
    try:
        async with websockets.connect(url, ping_interval=None) as websocket:
            print("Connected!")
            msg = await asyncio.wait_for(websocket.recv(), timeout=5.0)
            print(f"Msg: {msg[:100]}...")
            return True
    except Exception as e:
        print(f"Error: {e}")
        return False

async def main():
    urls = [
        "wss://fstream.binance.com/stream?streams=btcusdt@depth20@100ms",
        "wss://fstream.binance.com/market/stream?streams=btcusdt@depth20@100ms",
        "wss://fstream.binance.com/public/stream?streams=btcusdt@depth20@100ms",
        "wss://fstream.binance.com/ws/btcusdt@depth20@100ms"
    ]
    for url in urls:
        success = await test_depth(url)
        if success:
            print(f"SUCCESS: {url}")
            sys.exit(0)

if __name__ == "__main__":
    asyncio.run(main())

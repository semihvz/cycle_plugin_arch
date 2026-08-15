import asyncio
import websockets
import json

async def test_agg():
    uri = "wss://fstream.binance.com/stream?streams=btcusdt@aggTrade/ethusdt@aggTrade/aceusdt@aggTrade"
    print(f"Connecting to {uri}")
    try:
        async with websockets.connect(uri) as websocket:
            print("Connected!")
            for _ in range(3):
                msg = await websocket.recv()
                print(f"Msg: {msg}")
    except Exception as e:
        print(f"Error: {e}")

if __name__ == "__main__":
    asyncio.run(test_agg())

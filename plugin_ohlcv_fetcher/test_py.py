import asyncio
import websockets
import sys

async def test_binance():
    uri = "wss://fstream.binance.com/ws/btcusdt@ticker"
    print(f"Connecting to {uri} ...")
    try:
        async with websockets.connect(uri) as websocket:
            print("Connected!")
            for _ in range(3):
                msg = await websocket.recv()
                print(f"Msg: {msg}")
    except Exception as e:
        print(f"Error: {e}")

if __name__ == "__main__":
    asyncio.run(test_binance())

import asyncio
import websockets
import json

async def test_stream():
    uri = "wss://fstream.binance.com/stream?streams=btcusdt@forceOrder/ethusdt@forceOrder/aceusdt@forceOrder/bnbusdt@forceOrder/solusdt@forceOrder/xrpusdt@forceOrder"
    print("Connecting...")
    async with websockets.connect(uri) as ws:
        print("Connected!")
        while True:
            try:
                msg = await asyncio.wait_for(ws.recv(), timeout=60)
                print("Received:", msg)
            except asyncio.TimeoutError:
                print("Waiting...")

asyncio.run(test_stream())

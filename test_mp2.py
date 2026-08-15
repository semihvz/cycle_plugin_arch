import asyncio
import websockets

async def test():
    uri = "wss://fstream.binance.com/stream?streams=btcusdt@markPrice/btcusdt@markPrice@1s/BTCUSDT@markPrice/BTCUSDT@markPrice@1s/!markPrice@arr@1s"
    async with websockets.connect(uri) as websocket:
        print("Connected to streams")
        try:
            msg = await asyncio.wait_for(websocket.recv(), timeout=15)
            print("Received:", msg[:100])
        except Exception as e:
            print("Error/Timeout:", e)

asyncio.run(test())

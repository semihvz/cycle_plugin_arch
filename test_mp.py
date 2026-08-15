import asyncio
import websockets

async def test():
    uri = "wss://fstream.binance.com/ws/btcusdt@markPrice@1s"
    async with websockets.connect(uri) as websocket:
        print("Connected to markPrice@1s")
        try:
            msg = await asyncio.wait_for(websocket.recv(), timeout=10)
            print("Received:", msg)
        except Exception as e:
            print("Error/Timeout:", e)

asyncio.run(test())

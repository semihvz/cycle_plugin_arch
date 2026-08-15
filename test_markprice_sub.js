const WebSocket = require('ws');
const ws = new WebSocket('wss://fstream.binance.com/ws');

ws.on('open', () => {
  console.log('Connected, sending subscribe request');
  ws.send(JSON.stringify({
    "method": "SUBSCRIBE",
    "params": [
      "btcusdt@markPrice@1s",
      "btcusdt@markPrice"
    ],
    "id": 1
  }));
});
ws.on('message', data => { console.log('Message:', data.toString()); });
ws.on('error', err => { console.log('Error:', err); process.exit(1); });

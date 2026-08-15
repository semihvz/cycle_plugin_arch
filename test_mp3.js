const WebSocket = require('ws');
const ws = new WebSocket('wss://fstream.binance.com/market/ws/btcusdt@markPrice@1s');
ws.on('open', () => console.log('Connected market/ws markPrice@1s'));
ws.on('message', data => { console.log('Message:', data.toString()); process.exit(0); });
ws.on('error', err => { console.log('Error:', err); process.exit(1); });

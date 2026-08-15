const WebSocket = require('ws');
const ws = new WebSocket('wss://fstream.binance.com/ws/btcusdt@markPrice');

ws.on('open', () => console.log('Connected markPrice 3s'));
ws.on('message', data => { console.log('Message markPrice:', data.toString()); process.exit(0); });
ws.on('error', err => { console.log('Error:', err); process.exit(1); });

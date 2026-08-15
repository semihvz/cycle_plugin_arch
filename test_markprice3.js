const WebSocket = require('ws');
const ws = new WebSocket('wss://fstream.binance.com/stream?streams=btcusdt@markPrice@1s');

ws.on('open', () => console.log('Connected stream'));
ws.on('message', data => { console.log('Message stream:', data.toString()); process.exit(0); });
ws.on('error', err => { console.log('Error:', err); process.exit(1); });

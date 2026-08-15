const WebSocket = require('ws');
const ws = new WebSocket('wss://fstream.binance.com/ws/BTCUSDT@markPrice@1s');

ws.on('open', () => console.log('Connected markPrice upper'));
ws.on('message', data => { console.log('Message:', data.toString()); process.exit(0); });
ws.on('error', err => { console.log('Error:', err); process.exit(1); });

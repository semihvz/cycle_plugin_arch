const WebSocket = require('ws');
const ws = new WebSocket('wss://stream.binancefuture.com/ws/btcusdt@markPrice@1s');

ws.on('open', () => console.log('Connected testnet'));
ws.on('message', data => { console.log('Message testnet:', data.toString()); process.exit(0); });
ws.on('error', err => { console.log('Error:', err); process.exit(1); });

const WebSocket = require('ws');
const ws = new WebSocket('wss://fstream.binance.com/ws/btcusdt@depth20@100ms');

ws.on('open', () => console.log('Connected depth'));
ws.on('message', data => { console.log('Message depth:', data.toString()); process.exit(0); });

const WebSocket = require('ws');

const ws = new WebSocket('wss://fstream.binance.com/ws/btcusdt@bookTicker');
ws.on('open', () => console.log('Connected bookTicker'));
ws.on('message', data => { console.log('Message bookTicker:', data.toString()); process.exit(0); });

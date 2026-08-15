const WebSocket = require('ws');
const ws = new WebSocket('wss://fstream.binance.com/ws/btcusdt@trade');
ws.on('open', () => console.log('Connected futures trade'));
ws.on('message', data => { console.log('Message futures trade:', data.toString()); process.exit(0); });

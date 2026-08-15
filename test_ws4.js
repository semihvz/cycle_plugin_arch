const WebSocket = require('ws');

const ws = new WebSocket('wss://stream.binance.com:9443/ws/btcusdt@aggTrade');
ws.on('open', () => console.log('Connected spot aggTrade'));
ws.on('message', data => { console.log('Message spot:', data.toString()); process.exit(0); });

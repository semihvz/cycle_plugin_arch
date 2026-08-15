const WebSocket = require('ws');
const ws = new WebSocket('wss://fstream.binance.com/ws/!markPrice@arr@1s');

ws.on('open', () => console.log('Connected all markPrice'));
ws.on('message', data => { console.log('Message all:', data.toString().substring(0, 500)); process.exit(0); });
ws.on('error', err => { console.log('Error:', err); process.exit(1); });

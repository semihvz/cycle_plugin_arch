const WebSocket = require('ws');
const ws = new WebSocket('wss://fstream.binance.com/ws/btcusdt@aggTrade');

ws.on('open', function open() {
  console.log('Connected 1');
});

ws.on('message', function incoming(data) {
  console.log('Message 1:', data.toString());
  process.exit(0);
});

const ws2 = new WebSocket('wss://fstream.binance.com/stream?streams=btcusdt@aggTrade');

ws2.on('open', function open() {
  console.log('Connected 2');
});

ws2.on('message', function incoming(data) {
  console.log('Message 2:', data.toString());
  process.exit(0);
});

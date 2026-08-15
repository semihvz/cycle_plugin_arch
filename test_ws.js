const WebSocket = require('ws');
const ws = new WebSocket('wss://fstream.binance.com/ws/btcusdt@aggTrade');

ws.on('open', function open() {
  console.log('Connected');
});

ws.on('message', function incoming(data) {
  console.log('Message:', data.toString());
  process.exit(0);
});

ws.on('error', function error(err) {
  console.log('Error:', err);
  process.exit(1);
});

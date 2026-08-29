const mc = require('minecraft-protocol');

const client = mc.createClient({
  host: '127.0.0.1',
  port: 25565,
  username: 'potatosips',
  version: '1.21.4',
  auth: 'offline'
});

console.log('Connecting as potatosips to Pumpkin at 127.0.0.1:25565...');

client.on('raw', (buffer, meta) => {
  console.log(`[RAW PACKET] state: ${meta.state}, name: ${meta.name}, id: 0x${meta.id ? meta.id.toString(16) : '?'}, length: ${buffer.length}`);
});

client.on('error', (err) => {
  console.error('[CLIENT ERROR]:', err);
});

client.on('end', (reason) => {
  console.log('[CLIENT DISCONNECTED]:', reason);
});

const mc = require('minecraft-protocol');

const client = mc.createClient({
  host: '127.0.0.1',
  port: 25565,
  username: 'TestBot',
  version: '1.21.4',
  auth: 'offline'
});

client.on('packet', (data, meta) => {
  if (meta.name === 'declare_commands') {
    console.log('Successfully decoded declare_commands packet!');
    console.log('Total nodes count:', data.nodes.length);
    console.log('Root node index:', data.rootIndex);
    process.exit(0);
  }
});

client.on('error', (err) => {
  console.error('Error:', err);
});

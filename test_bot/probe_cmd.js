const mc = require('minecraft-protocol');
const client = mc.createClient({ host: '127.0.0.1', port: 25565, username: 'TestBot', version: '1.21.4', auth: 'offline' });
client.on('position', () => {
  client.write('chat_command', { command: 'execute if block 180 69 28 minecraft:scaffolding run say PUMPKIN_PLAIN_MATCH', timestamp: BigInt(Date.now()) });
  client.write('chat_command', { command: 'execute if block 180 69 28 minecraft:scaffolding[bottom=false,distance=0] run say PUMPKIN_PROP_MATCH', timestamp: BigInt(Date.now()) });
  setTimeout(() => client.end(), 1000);
});
client.on('system_chat', packet => console.log('SYS:', JSON.stringify(packet.content)));
client.on('profileless_chat', packet => console.log('PROF:', JSON.stringify(packet.message)));
client.on('disguised_chat', packet => console.log('DISG:', JSON.stringify(packet.message)));

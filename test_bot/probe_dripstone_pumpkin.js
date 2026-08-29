const mc = require('minecraft-protocol');
const client = mc.createClient({ host: '127.0.0.1', port: 25565, username: 'TestBot', version: '1.21.4', auth: 'offline' });
client.on('position', () => {
  client.write('chat_command', { command: 'execute if block 178 79 28 minecraft:pointed_dripstone run say HAS_79', timestamp: BigInt(Date.now()) });
  client.write('chat_command', { command: 'execute if block 178 78 28 minecraft:pointed_dripstone run say HAS_78', timestamp: BigInt(Date.now()) });
  client.write('chat_command', { command: 'execute if block 178 77 28 minecraft:pointed_dripstone run say HAS_77', timestamp: BigInt(Date.now()) });
  setTimeout(() => client.end(), 1000);
});
client.on('system_chat', p => console.log('SYS:', JSON.stringify(p.content)));
client.on('profileless_chat', p => console.log('PROF:', JSON.stringify(p.message)));
client.on('disguised_chat', p => console.log('DISG:', JSON.stringify(p.message)));

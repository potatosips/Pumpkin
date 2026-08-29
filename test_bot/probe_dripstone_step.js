const mc = require('minecraft-protocol');
const client = mc.createClient({ host: '127.0.0.1', port: 25565, username: 'TestBot', version: '1.21.4', auth: 'offline' });
client.on('position', () => {
  client.write('chat_command', { command: 'fill 178 70 28 178 85 28 air', timestamp: BigInt(Date.now()) });
  client.write('chat_command', { command: 'setblock 178 80 28 minecraft:stone', timestamp: BigInt(Date.now()) });
  client.write('chat_command', { command: 'setblock 178 79 28 minecraft:pointed_dripstone[vertical_direction=down]', timestamp: BigInt(Date.now()) });
  client.write('chat_command', { command: 'setblock 178 78 28 minecraft:pointed_dripstone[vertical_direction=down]', timestamp: BigInt(Date.now()) });
  client.write('chat_command', { command: 'setblock 178 77 28 minecraft:pointed_dripstone[vertical_direction=down]', timestamp: BigInt(Date.now()) });
  setTimeout(() => {
    client.write('chat_command', { command: 'execute if block 178 79 28 minecraft:pointed_dripstone run say 79_OK', timestamp: BigInt(Date.now()) });
    client.write('chat_command', { command: 'execute if block 178 78 28 minecraft:pointed_dripstone run say 78_OK', timestamp: BigInt(Date.now()) });
    client.write('chat_command', { command: 'execute if block 178 77 28 minecraft:pointed_dripstone run say 77_OK', timestamp: BigInt(Date.now()) });
  }, 1000);
  setTimeout(() => client.end(), 2000);
});
client.on('system_chat', p => console.log('SYS:', JSON.stringify(p.content)));
client.on('profileless_chat', p => console.log('PROF:', JSON.stringify(p.message)));
client.on('disguised_chat', p => console.log('DISG:', JSON.stringify(p.message)));

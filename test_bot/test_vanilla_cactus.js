const mc = require('minecraft-protocol');

const client = mc.createClient({ host: '127.0.0.1', port: 25575, username: 'TestBot', version: '1.21.4', auth: 'offline' });
client.on('position', () => {
  setTimeout(() => {
    client.write('chat_command', { command: 'tp @s 230 75 28', timestamp: BigInt(Date.now()) });
    client.write('chat_command', { command: 'fill 228 68 26 232 75 30 air', timestamp: BigInt(Date.now()) });
    setTimeout(() => {
      client.write('chat_command', { command: 'setblock 230 69 28 minecraft:sand', timestamp: BigInt(Date.now()) });
      client.write('chat_command', { command: 'setblock 230 70 28 minecraft:cactus', timestamp: BigInt(Date.now()) });
    }, 500);
    setTimeout(() => {
      client.write('chat_command', { command: 'execute if block 230 70 28 minecraft:cactus run say FOUND_ON_VANILLA', timestamp: BigInt(Date.now()) });
    }, 1200);
    setTimeout(() => client.end(), 2000);
  }, 500);
});

client.on('system_chat', packet => console.log('[SYS]', packet.content));
client.on('player_chat', packet => console.log('[PLAYER_CHAT]', packet.plainMessage || packet));

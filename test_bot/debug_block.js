const mc = require('minecraft-protocol');

function inspect(port, label) {
  const client = mc.createClient({ host: '127.0.0.1', port, username: 'TestBot', version: '1.21.4', auth: 'offline' });
  client.on('position', () => {
    setTimeout(() => {
      client.write('chat_command', { command: 'tp @s 178 75 28', timestamp: BigInt(Date.now()) });
      client.write('chat_command', { command: 'fill 176 68 26 180 75 30 air', timestamp: BigInt(Date.now()) });
      setTimeout(() => {
        client.write('chat_command', { command: 'setblock 178 69 28 minecraft:sand', timestamp: BigInt(Date.now()) });
        client.write('chat_command', { command: 'setblock 178 70 28 minecraft:cactus', timestamp: BigInt(Date.now()) });
      }, 500);
      setTimeout(() => {
        client.write('chat_command', { command: 'execute if block 178 70 28 minecraft:cactus run say FOUND_CACTUS', timestamp: BigInt(Date.now()) });
        client.write('chat_command', { command: 'execute if block 178 69 28 minecraft:sand run say FOUND_SAND', timestamp: BigInt(Date.now()) });
      }, 1000);
      setTimeout(() => client.end(), 2000);
    }, 500);
  });
  client.on('system_chat', packet => console.log(`[${label} SYS]`, packet.content));
  client.on('player_chat', packet => console.log(`[${label} CHAT]`, packet.plainMessage || packet));
}

inspect(25565, 'PUMPKIN');
setTimeout(() => inspect(25575, 'VANILLA'), 300);

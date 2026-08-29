const mc = require('minecraft-protocol');

const client = mc.createClient({ host: '127.0.0.1', port: 25565, username: 'TestBot', version: '1.21.4', auth: 'offline' });
client.on('position', () => {
  setTimeout(() => {
    client.write('chat_command', { command: 'tp @s 182 85 28', timestamp: BigInt(Date.now()) });
    client.write('chat_command', { command: 'fill 175 68 25 190 85 32 air', timestamp: BigInt(Date.now()) });
    client.write('chat_command', { command: 'fill 175 68 25 190 68 32 stone', timestamp: BigInt(Date.now()) });
    client.write('chat_command', { command: 'setblock 180 75 28 stone', timestamp: BigInt(Date.now()) });
    client.write('chat_command', { command: 'setblock 180 76 28 minecraft:dragon_egg', timestamp: BigInt(Date.now()) });
    setTimeout(() => {
      client.write('chat_command', { command: 'setblock 180 75 28 air', timestamp: BigInt(Date.now()) });
      for (let t = 1; t <= 10; t++) {
        setTimeout(() => {
          client.write('chat_command', { command: `say TICK_${t}`, timestamp: BigInt(Date.now()) });
          client.write('chat_command', { command: `data get entity @e[type=falling_block,limit=1] Pos`, timestamp: BigInt(Date.now()) });
        }, t * 200);
      }
      setTimeout(() => {
        for (let y = 68; y <= 77; y++) {
          client.write('chat_command', { command: `execute if block 180 ${y} 28 minecraft:dragon_egg run say EGG_AT_${y}`, timestamp: BigInt(Date.now()) });
        }
      }, 2500);
      setTimeout(() => client.end(), 3500);
    }, 1000);
  }, 500);
});

client.on('system_chat', packet => console.log(JSON.stringify(packet.content)));
client.on('profileless_chat', packet => console.log(JSON.stringify(packet.message)));

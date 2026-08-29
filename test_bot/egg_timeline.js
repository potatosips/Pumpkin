const mc = require('minecraft-protocol');

const client = mc.createClient({ host: '127.0.0.1', port: 25565, username: 'TestBot', version: '1.21.4', auth: 'offline' });
client.on('position', () => {
  setTimeout(() => {
    client.write('chat_command', { command: 'tp @s 182 85 28', timestamp: BigInt(Date.now()) });
    client.write('chat_command', { command: 'fill 175 68 25 190 85 32 air', timestamp: BigInt(Date.now()) });
    client.write('chat_command', { command: 'fill 175 68 25 190 68 32 stone', timestamp: BigInt(Date.now()) });
    setTimeout(() => {
      client.write('chat_command', { command: 'setblock 180 75 28 stone', timestamp: BigInt(Date.now()) });
      client.write('chat_command', { command: 'setblock 180 76 28 minecraft:dragon_egg', timestamp: BigInt(Date.now()) });
      setTimeout(() => {
        client.write('chat_command', { command: 'setblock 180 75 28 air', timestamp: BigInt(Date.now()) });
        for (let delay = 500; delay <= 4000; delay += 500) {
          setTimeout(() => {
            client.write('chat_command', { command: `say CHECK_AT_${delay}`, timestamp: BigInt(Date.now()) });
            client.write('chat_command', { command: `execute if block 180 69 28 minecraft:dragon_egg run say EGG_AT_69_DELAY_${delay}`, timestamp: BigInt(Date.now()) });
            client.write('chat_command', { command: `execute if block 180 76 28 minecraft:dragon_egg run say EGG_AT_76_DELAY_${delay}`, timestamp: BigInt(Date.now()) });
            client.write('chat_command', { command: `execute as @e[type=falling_block] run say FALLING_ENTITY_EXISTS`, timestamp: BigInt(Date.now()) });
          }, delay);
        }
        setTimeout(() => client.end(), 4500);
      }, 1000);
    }, 1000);
  }, 500);
});

client.on('system_chat', packet => console.log(JSON.stringify(packet.content)));
client.on('profileless_chat', packet => console.log(JSON.stringify(packet.message)));

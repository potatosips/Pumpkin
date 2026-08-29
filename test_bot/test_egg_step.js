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
        setTimeout(() => {
          client.write('chat_command', { command: 'execute if block 180 69 28 minecraft:dragon_egg run say PASS_DRAGON_EGG_LANDED', timestamp: BigInt(Date.now()) });
          client.write('chat_command', { command: 'execute if block 180 76 28 minecraft:dragon_egg run say AT_76', timestamp: BigInt(Date.now()) });
          client.write('chat_command', { command: 'execute if block 180 68 28 minecraft:dragon_egg run say AT_68', timestamp: BigInt(Date.now()) });
          client.write('chat_command', { command: 'execute as @e[type=falling_block] run say FALLING_ALIVE', timestamp: BigInt(Date.now()) });
          client.write('chat_command', { command: 'execute as @e[type=item] run say ITEM_ALIVE', timestamp: BigInt(Date.now()) });
          setTimeout(() => client.end(), 1000);
        }, 3000);
      }, 1000);
    }, 1000);
  }, 500);
});

client.on('profileless_chat', packet => console.log('PROFILELESS:', JSON.stringify(packet)));
client.on('system_chat', packet => console.log('SYSTEM:', JSON.stringify(packet)));

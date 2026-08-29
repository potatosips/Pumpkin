const mc = require('minecraft-protocol');

function run(name, port) {
  const client = mc.createClient({ host: '127.0.0.1', port, username: 'TestBot', version: '1.21.4', auth: 'offline' });
  client.on('position', () => {
    setTimeout(() => {
      client.write('chat_command', { command: 'setblock 180 76 28 air', timestamp: BigInt(Date.now()) });
      setTimeout(() => {
        client.write('chat_command', { command: `execute if entity @e[type=falling_block] run say [${name}] FALLING_ENTITY_EXISTS`, timestamp: BigInt(Date.now()) });
      }, 300);
      setTimeout(() => {
        for (let y = 68; y <= 77; y++) {
          client.write('chat_command', { command: `execute if block 180 ${y} 28 minecraft:dragon_egg run say [${name}] EGG_AT_${y}`, timestamp: BigInt(Date.now()) });
        }
      }, 1500);
      setTimeout(() => client.end(), 2500);
    }, 500);
  });
  client.on('system_chat', packet => console.log(JSON.stringify(packet.content)));
  client.on('profileless_chat', packet => console.log(JSON.stringify(packet.message)));
}

run('PUMPKIN', 25565);
run('VANILLA', 25575);

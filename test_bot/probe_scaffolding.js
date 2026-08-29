const mc = require('minecraft-protocol');

const client = mc.createClient({
  host: '127.0.0.1',
  port: 25575,
  username: 'TestBot',
  version: '1.21.4',
  auth: 'offline'
});

client.on('position', () => {
  console.log('TestBot connected to Vanilla!');
  const commands = [
    'tp @s 182 85 28',
    'fill 175 68 25 190 85 32 air',
    'fill 175 68 25 190 68 32 stone',
    
    // Pillar: bottom on stone -> distance=0, bottom=true? Let's check
    'setblock 180 69 28 minecraft:scaffolding',
    'setblock 180 70 28 minecraft:scaffolding',
    
    // Horizontal branch from (180, 70, 28):
    'setblock 181 70 28 minecraft:scaffolding',
    'setblock 182 70 28 minecraft:scaffolding',
    'setblock 183 70 28 minecraft:scaffolding',
    'setblock 184 70 28 minecraft:scaffolding',
    'setblock 185 70 28 minecraft:scaffolding',
    'setblock 186 70 28 minecraft:scaffolding',
  ];

  commands.forEach((command, idx) => {
    setTimeout(() => {
      client.write('chat_command', { command, timestamp: BigInt(Date.now()) });
    }, idx * 100);
  });

  setTimeout(() => {
    // Query states
    client.write('chat_command', { command: 'execute if block 180 69 28 minecraft:scaffolding[bottom=true,distance=0] run say SCAF_180_69_BOTTOM_TRUE_DIST_0', timestamp: BigInt(Date.now()) });
    client.write('chat_command', { command: 'execute if block 180 70 28 minecraft:scaffolding[bottom=false,distance=0] run say SCAF_180_70_BOTTOM_FALSE_DIST_0', timestamp: BigInt(Date.now()) });
    client.write('chat_command', { command: 'execute if block 181 70 28 minecraft:scaffolding[bottom=true,distance=1] run say SCAF_181_70_BOTTOM_TRUE_DIST_1', timestamp: BigInt(Date.now()) });
    client.write('chat_command', { command: 'execute if block 186 70 28 minecraft:scaffolding[bottom=true,distance=6] run say SCAF_186_70_BOTTOM_TRUE_DIST_6', timestamp: BigInt(Date.now()) });
  }, commands.length * 100 + 800);

  setTimeout(() => client.end(), commands.length * 100 + 2000);
});

client.on('system_chat', packet => console.log('[VANILLA]', JSON.stringify(packet.content)));
client.on('profileless_chat', packet => console.log('[VANILLA]', JSON.stringify(packet.message)));
client.on('disguised_chat', packet => console.log('[VANILLA]', JSON.stringify(packet.message)));

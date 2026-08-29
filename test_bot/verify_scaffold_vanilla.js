const mc = require('minecraft-protocol');

const client = mc.createClient({
  host: '127.0.0.1',
  port: 25575,
  username: 'TestBot',
  version: '1.21.4',
  auth: 'offline'
});

client.on('position', () => {
  const commands = [
    'tp @s 182 85 28',
    'fill 175 68 25 190 85 32 air',
    'fill 175 68 25 190 68 32 stone',
    
    // Test 1: Column on stone
    'setblock 180 69 28 minecraft:scaffolding', // distance=0, bottom=false
    'setblock 180 70 28 minecraft:scaffolding', // distance=0, bottom=false
    
    // Test 2: Horizontal branch
    'setblock 181 70 28 minecraft:scaffolding', // distance=1, bottom=true
    'setblock 182 70 28 minecraft:scaffolding', // distance=2, bottom=true
    'setblock 183 70 28 minecraft:scaffolding', // distance=3, bottom=true
    'setblock 184 70 28 minecraft:scaffolding', // distance=4, bottom=true
    'setblock 185 70 28 minecraft:scaffolding', // distance=5, bottom=true
    'setblock 186 70 28 minecraft:scaffolding', // distance=6, bottom=true
    'setblock 187 70 28 minecraft:scaffolding', // distance=7 -> falls or breaks!
  ];

  commands.forEach((command, idx) => {
    setTimeout(() => {
      client.write('chat_command', { command, timestamp: BigInt(Date.now()) });
    }, idx * 100);
  });

  const queryStart = commands.length * 100 + 1000;
  const queries = [
    'execute if block 180 69 28 minecraft:scaffolding[bottom=false,distance=0] run say PASS_COL_BASE_0',
    'execute if block 180 70 28 minecraft:scaffolding[bottom=false,distance=0] run say PASS_COL_TOP_0',
    'execute if block 181 70 28 minecraft:scaffolding[bottom=true,distance=1] run say PASS_BRANCH_1',
    'execute if block 182 70 28 minecraft:scaffolding[bottom=true,distance=2] run say PASS_BRANCH_2',
    'execute if block 183 70 28 minecraft:scaffolding[bottom=true,distance=3] run say PASS_BRANCH_3',
    'execute if block 184 70 28 minecraft:scaffolding[bottom=true,distance=4] run say PASS_BRANCH_4',
    'execute if block 185 70 28 minecraft:scaffolding[bottom=true,distance=5] run say PASS_BRANCH_5',
    'execute if block 186 70 28 minecraft:scaffolding[bottom=true,distance=6] run say PASS_BRANCH_6',
    'execute if block 187 70 28 minecraft:scaffolding run say AT_187_70',
    'execute if block 187 69 28 minecraft:scaffolding run say AT_187_69',
  ];

  queries.forEach((command, idx) => {
    setTimeout(() => {
      client.write('chat_command', { command, timestamp: BigInt(Date.now()) });
    }, queryStart + idx * 100);
  });

  setTimeout(() => client.end(), queryStart + queries.length * 100 + 1000);
});

client.on('system_chat', packet => console.log('[VANILLA SYS]', JSON.stringify(packet.content)));
client.on('profileless_chat', packet => console.log('[VANILLA PROF]', JSON.stringify(packet.message)));
client.on('disguised_chat', packet => console.log('[VANILLA DISG]', JSON.stringify(packet.message)));
client.on('player_chat', packet => console.log('[VANILLA CHAT]', JSON.stringify(packet)));

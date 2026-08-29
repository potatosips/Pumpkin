const mc = require('minecraft-protocol');

function buildSetup() {
  return [
    'tp @s 915 85 28',
    'kill @e[type=item,x=900,y=60,z=20,dx=45,dy=35,dz=15]',
    'fill 900 67 25 945 76 31 air',
    'fill 900 68 25 945 68 31 minecraft:stone',
    
    // Foundations
    'setblock 902 69 28 minecraft:tube_coral_block',
    'setblock 902 71 28 minecraft:water',
    
    'setblock 906 69 28 minecraft:tube_coral_block',
    'setblock 906 70 28 minecraft:water',
    
    'setblock 910 69 28 minecraft:tube_coral_block',
    'setblock 910 70 28 minecraft:water',
    
    'setblock 914 70 27 minecraft:tube_coral_block',
    'setblock 914 70 28 minecraft:water',
    
    'setblock 918 69 28 minecraft:brain_coral_block',
    'setblock 918 71 28 minecraft:water',
    
    'setblock 922 69 28 minecraft:stone',
    
    'setblock 926 69 28 minecraft:tube_coral_block',
    'setblock 926 70 28 minecraft:water',
  ];
}

const placementPhase = [
    'setblock 902 70 28 minecraft:tube_coral_block',
    'setblock 906 70 28 minecraft:tube_coral[waterlogged=true]',
    'setblock 910 70 28 minecraft:tube_coral_fan[waterlogged=true]',
    'setblock 914 70 28 minecraft:tube_coral_wall_fan[facing=south,waterlogged=true]',
    'setblock 918 70 28 minecraft:brain_coral_block',
    'setblock 922 70 28 minecraft:dead_tube_coral_block',
    'setblock 926 70 28 minecraft:tube_coral[waterlogged=true]',
];

const breakPhase = [
    'setblock 926 69 28 minecraft:air',
];

const verify = [
  'execute if block 902 70 28 minecraft:tube_coral_block run say PASS_CORAL_BLOCK_SUBMERGED',
  'execute if block 906 70 28 minecraft:tube_coral run say PASS_CORAL_PLANT_ON_CORAL_BLOCK',
  'execute if block 910 70 28 minecraft:tube_coral_fan run say PASS_CORAL_FAN_ON_CORAL_BLOCK',
  'execute if block 914 70 28 minecraft:tube_coral_wall_fan run say PASS_CORAL_WALL_FAN_ON_CORAL_BLOCK',
  'execute if block 918 70 28 minecraft:brain_coral_block run say PASS_BRAIN_CORAL_BLOCK_SUBMERGED',
  'execute if block 922 70 28 minecraft:dead_tube_coral_block run say PASS_DEAD_TUBE_CORAL_BLOCK',
  'execute unless block 926 70 28 minecraft:tube_coral run say PASS_SUPPORT_REMOVAL_BREAK',
];

let finished = 0;
const results = { PUMPKIN: [], VANILLA: [] };

function summarize(node) {
  if (node == null) return '';
  if (typeof node !== 'object') return String(node);
  if (node.type && node.type !== 'compound' && node.type !== 'list') return summarize(node.value);
  if (node.type === 'list') return (node.value?.value ?? node.value ?? []).map(summarize).join('');
  if (node.type === 'compound') return Object.values(node.value ?? {}).map(summarize).filter(Boolean).join('|');
  return Object.values(node).map(summarize).filter(Boolean).join('|');
}

function handleMsg(name, raw) {
  const text = typeof raw === 'string' ? raw : summarize(raw);
  if (text.startsWith('red|') || text.includes('command.context.here')) {
    return;
  }
  if (text.includes('PASS_')) {
    results[name].push(text);
    console.log(`[${name}] ${text}`);
  }
}

function run(name, port) {
  const client = mc.createClient({ host: '127.0.0.1', port, username: 'TestBot', version: '1.21.4', auth: 'offline' });
  let sent = false;
  client.on('position', () => {
    if (sent) return;
    sent = true;
    setTimeout(() => {
      const setup = buildSetup();
      setup.forEach((command, index) => setTimeout(() => {
        client.write('chat_command', { command, timestamp: BigInt(Date.now()) });
      }, index * 150));

      const placeStart = setup.length * 150 + 2000;
      placementPhase.forEach((command, index) => setTimeout(() => {
        client.write('chat_command', { command, timestamp: BigInt(Date.now()) });
      }, placeStart + index * 200));

      const breakStart = placeStart + placementPhase.length * 200 + 2000;
      breakPhase.forEach((command, index) => setTimeout(() => {
        client.write('chat_command', { command, timestamp: BigInt(Date.now()) });
      }, breakStart + index * 200));

      const verifyStart = breakStart + breakPhase.length * 200 + 2000;
      verify.forEach((command, index) => setTimeout(() => {
        client.write('chat_command', { command, timestamp: BigInt(Date.now()) });
      }, verifyStart + index * 200));

      setTimeout(() => client.end(), verifyStart + verify.length * 200 + 1500);
    }, 500);
  });

  client.on('system_chat', packet => handleMsg(name, packet.content));
  client.on('profileless_chat', packet => handleMsg(name, packet.message));
  client.on('disguised_chat', packet => handleMsg(name, packet.message));
  client.on('player_chat', packet => handleMsg(name, packet.unsignedContent || packet.plainMessage || packet.signedChatContent || packet));

  client.on('error', error => console.error(`[${name}] ERROR ${error.message}`));
  client.on('end', () => {
    if (++finished === 2) {
      console.log('\n=== CORAL BLOCKS, FANS & PLANTS DUAL-SERVER DIFFERENTIAL SUMMARY ===');
      const expected = [
        'PASS_CORAL_BLOCK_SUBMERGED',
        'PASS_CORAL_PLANT_ON_CORAL_BLOCK',
        'PASS_CORAL_FAN_ON_CORAL_BLOCK',
        'PASS_CORAL_WALL_FAN_ON_CORAL_BLOCK',
        'PASS_BRAIN_CORAL_BLOCK_SUBMERGED',
        'PASS_DEAD_TUBE_CORAL_BLOCK',
        'PASS_SUPPORT_REMOVAL_BREAK',
      ];
      let matchCount = 0;
      for (const exp of expected) {
        const pHas = results.PUMPKIN.some(l => l.includes(exp));
        const vHas = results.VANILLA.some(l => l.includes(exp));
        const matched = pHas && vHas;
        if (matched) matchCount++;
        console.log(`[TEST: ${exp}]`);
        console.log(`  Pumpkin: ${pHas ? 'PASSED (MATCH)' : 'FAILED'}`);
        console.log(`  Vanilla: ${vHas ? 'PASSED (MATCH)' : 'FAILED'}`);
        console.log(`  Status:  ${matched ? '100% PARITY' : 'MISMATCH'}\n`);
      }
      console.log(`Total Parity Score: ${matchCount}/${expected.length} (${matchCount === expected.length ? '100% PARITY' : 'MISMATCH'})`);
      process.exit(matchCount === expected.length ? 0 : 1);
    }
  });
}

run('PUMPKIN', 25565);
setTimeout(() => run('VANILLA', 25575), 200);

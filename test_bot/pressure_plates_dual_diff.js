const mc = require('minecraft-protocol');

function buildSetup() {
  return [
    'tp @s 1365 85 28',
    'kill @e[type=item,x=1350,y=60,z=20,dx=45,dy=35,dz=15]',
    'fill 1350 67 25 1395 76 31 air',
    'fill 1350 68 25 1395 68 31 minecraft:stone',
    
    // Foundations
    'setblock 1352 69 28 minecraft:stone',
    'setblock 1356 69 28 minecraft:stone',
    'setblock 1360 69 28 minecraft:stone',
    'setblock 1364 69 28 minecraft:stone',
    'setblock 1368 69 28 minecraft:stone',
    'setblock 1372 69 28 minecraft:stone',
  ];
}

const placementPhase = [
    'setblock 1352 70 28 minecraft:stone_pressure_plate',
    'setblock 1356 70 28 minecraft:oak_pressure_plate',
    'setblock 1360 70 28 minecraft:light_weighted_pressure_plate',
    'setblock 1364 70 28 minecraft:heavy_weighted_pressure_plate',
    'setblock 1368 70 28 minecraft:polished_blackstone_pressure_plate',
    'setblock 1372 70 28 minecraft:stone_pressure_plate',
];

const breakPhase = [
    'setblock 1372 69 28 minecraft:air',
];

const verify = [
  'execute if block 1352 70 28 minecraft:stone_pressure_plate run say PASS_STONE_PRESSURE_PLATE',
  'execute if block 1356 70 28 minecraft:oak_pressure_plate run say PASS_OAK_PRESSURE_PLATE',
  'execute if block 1360 70 28 minecraft:light_weighted_pressure_plate run say PASS_LIGHT_WEIGHTED_PRESSURE_PLATE',
  'execute if block 1364 70 28 minecraft:heavy_weighted_pressure_plate run say PASS_HEAVY_WEIGHTED_PRESSURE_PLATE',
  'execute if block 1368 70 28 minecraft:polished_blackstone_pressure_plate run say PASS_POLISHED_BLACKSTONE_PRESSURE_PLATE',
  'execute unless block 1372 70 28 minecraft:stone_pressure_plate run say PASS_PRESSURE_PLATE_SUPPORT_REMOVAL_BREAK',
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
      console.log('\n=== PRESSURE PLATES DUAL-SERVER DIFFERENTIAL SUMMARY ===');
      const expected = [
        'PASS_STONE_PRESSURE_PLATE',
        'PASS_OAK_PRESSURE_PLATE',
        'PASS_LIGHT_WEIGHTED_PRESSURE_PLATE',
        'PASS_HEAVY_WEIGHTED_PRESSURE_PLATE',
        'PASS_POLISHED_BLACKSTONE_PRESSURE_PLATE',
        'PASS_PRESSURE_PLATE_SUPPORT_REMOVAL_BREAK',
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

const mc = require('minecraft-protocol');

function buildSetup() {
  return [
    'tp @s 310 85 28',
    'kill @e[type=item,x=300,y=60,z=20,dx=25,dy=35,dz=15]',
    'fill 300 67 25 325 76 31 air',
    'fill 300 68 25 325 68 31 minecraft:stone',
    
    // Test 1: Seagrass on dirt underwater
    'setblock 302 69 28 minecraft:dirt',
    'setblock 302 70 28 minecraft:water',
    
    // Test 2: Seagrass on sand underwater
    'setblock 306 69 28 minecraft:sand',
    'setblock 306 70 28 minecraft:water',
    
    // Test 3: Seagrass on gravel underwater
    'setblock 310 69 28 minecraft:gravel',
    'setblock 310 70 28 minecraft:water',
    
    // Test 4 & 5: Tall seagrass (2-block tall underwater column)
    'setblock 314 69 28 minecraft:sand',
    'fill 314 70 28 314 71 28 minecraft:water',
    
    // Test 6: Support removal
    'setblock 318 69 28 minecraft:sand',
    'setblock 318 70 28 minecraft:water',
  ];
}

const placementPhase = [
    'setblock 302 70 28 minecraft:seagrass',
    'setblock 306 70 28 minecraft:seagrass',
    'setblock 310 70 28 minecraft:seagrass',
    'setblock 314 70 28 minecraft:tall_seagrass[half=lower]',
    'setblock 314 71 28 minecraft:tall_seagrass[half=upper]',
    'setblock 318 70 28 minecraft:seagrass',
];

const breakPhase = [
    'setblock 318 69 28 minecraft:air',
];

const verify = [
  'execute if block 302 70 28 minecraft:seagrass run say PASS_SEAGRASS_ON_DIRT_WATER',
  'execute if block 306 70 28 minecraft:seagrass run say PASS_SEAGRASS_ON_SAND_WATER',
  'execute if block 310 70 28 minecraft:seagrass run say PASS_SEAGRASS_ON_GRAVEL_WATER',
  'execute if block 314 70 28 minecraft:tall_seagrass run say PASS_TALL_SEAGRASS_LOWER',
  'execute if block 314 71 28 minecraft:tall_seagrass run say PASS_TALL_SEAGRASS_UPPER',
  'execute unless block 318 70 28 minecraft:seagrass run say PASS_SUPPORT_REMOVAL_BREAK',
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
    // Ignore error messages
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
      console.log('\n=== SEAGRASS DUAL-SERVER DIFFERENTIAL SUMMARY ===');
      const expected = [
        'PASS_SEAGRASS_ON_DIRT_WATER',
        'PASS_SEAGRASS_ON_SAND_WATER',
        'PASS_SEAGRASS_ON_GRAVEL_WATER',
        'PASS_TALL_SEAGRASS_LOWER',
        'PASS_TALL_SEAGRASS_UPPER',
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

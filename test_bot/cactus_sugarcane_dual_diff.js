const mc = require('minecraft-protocol');

function buildSetup() {
  return [
    'tp @s 495 85 28',
    'kill @e[type=item,x=480,y=60,z=20,dx=40,dy=35,dz=15]',
    'fill 480 67 25 520 76 31 air',
    'fill 480 68 25 520 68 31 minecraft:stone',
    
    // Test 1: Cactus on sand
    'setblock 482 69 28 minecraft:sand',
    
    // Test 2: Cactus on red sand
    'setblock 486 69 28 minecraft:red_sand',
    
    // Test 3: Cactus stack base and top
    'setblock 490 69 28 minecraft:sand',
    
    // Test 4: Sugar cane on sand adjacent to water
    'setblock 494 69 28 minecraft:sand',
    'setblock 494 69 29 minecraft:water',
    
    // Test 5: Sugar cane on red sand adjacent to water
    'setblock 498 69 28 minecraft:red_sand',
    'setblock 498 69 29 minecraft:water',
    
    // Test 6: Sugar cane on dirt adjacent to water
    'setblock 502 69 28 minecraft:dirt',
    'setblock 502 69 29 minecraft:water',
    
    // Test 7: Sugar cane stack
    'setblock 506 69 28 minecraft:sand',
    'setblock 506 69 29 minecraft:water',
    
    // Test 8: Support removal
    'setblock 510 69 28 minecraft:sand',
  ];
}

const placementPhase = [
    'setblock 482 70 28 minecraft:cactus',
    'setblock 486 70 28 minecraft:cactus',
    'setblock 490 70 28 minecraft:cactus',
    'setblock 490 71 28 minecraft:cactus',
    'setblock 494 70 28 minecraft:sugar_cane',
    'setblock 498 70 28 minecraft:sugar_cane',
    'setblock 502 70 28 minecraft:sugar_cane',
    'setblock 506 70 28 minecraft:sugar_cane',
    'setblock 506 71 28 minecraft:sugar_cane',
    'setblock 510 70 28 minecraft:cactus',
];

const breakPhase = [
    'setblock 510 69 28 minecraft:air',
];

const verify = [
  'execute if block 482 70 28 minecraft:cactus run say PASS_CACTUS_ON_SAND',
  'execute if block 486 70 28 minecraft:cactus run say PASS_CACTUS_ON_RED_SAND',
  'execute if block 490 70 28 minecraft:cactus run say PASS_CACTUS_STACK_BASE',
  'execute if block 490 71 28 minecraft:cactus run say PASS_CACTUS_STACK_TOP',
  'execute if block 494 70 28 minecraft:sugar_cane run say PASS_SUGARCANE_ON_SAND_ADJ_WATER',
  'execute if block 498 70 28 minecraft:sugar_cane run say PASS_SUGARCANE_ON_RED_SAND_ADJ_WATER',
  'execute if block 502 70 28 minecraft:sugar_cane run say PASS_SUGARCANE_ON_DIRT_ADJ_WATER',
  'execute if block 506 70 28 minecraft:sugar_cane run say PASS_SUGARCANE_STACK_BASE',
  'execute if block 506 71 28 minecraft:sugar_cane run say PASS_SUGARCANE_STACK_TOP',
  'execute unless block 510 70 28 minecraft:cactus run say PASS_SUPPORT_REMOVAL_BREAK',
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
      console.log('\n=== CACTUS & SUGARCANE DUAL-SERVER DIFFERENTIAL SUMMARY ===');
      const expected = [
        'PASS_CACTUS_ON_SAND',
        'PASS_CACTUS_ON_RED_SAND',
        'PASS_CACTUS_STACK_BASE',
        'PASS_CACTUS_STACK_TOP',
        'PASS_SUGARCANE_ON_SAND_ADJ_WATER',
        'PASS_SUGARCANE_ON_RED_SAND_ADJ_WATER',
        'PASS_SUGARCANE_ON_DIRT_ADJ_WATER',
        'PASS_SUGARCANE_STACK_BASE',
        'PASS_SUGARCANE_STACK_TOP',
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

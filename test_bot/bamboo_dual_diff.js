const mc = require('minecraft-protocol');

function buildSetup() {
  return [
    'tp @s 535 85 28',
    'kill @e[type=item,x=520,y=60,z=20,dx=40,dy=35,dz=15]',
    'fill 520 67 25 560 76 31 air',
    'fill 520 68 25 560 68 31 minecraft:stone',
    
    // Test 1: Bamboo on grass
    'setblock 522 69 28 minecraft:grass_block',
    
    // Test 2: Bamboo on dirt
    'setblock 526 69 28 minecraft:dirt',
    
    // Test 3: Bamboo on sand
    'setblock 530 69 28 minecraft:sand',
    
    // Test 4: Bamboo on gravel
    'setblock 534 69 28 minecraft:gravel',
    
    // Test 5: Bamboo sapling on dirt
    'setblock 538 69 28 minecraft:dirt',
    
    // Test 6: Bamboo sapling on sand
    'setblock 542 69 28 minecraft:sand',
    
    // Test 7 & 8: Bamboo stack
    'setblock 546 69 28 minecraft:grass_block',
    
    // Test 9: Support removal
    'setblock 550 69 28 minecraft:dirt',
  ];
}

const placementPhase = [
    'setblock 522 70 28 minecraft:bamboo',
    'setblock 526 70 28 minecraft:bamboo',
    'setblock 530 70 28 minecraft:bamboo',
    'setblock 534 70 28 minecraft:bamboo',
    'setblock 538 70 28 minecraft:bamboo_sapling',
    'setblock 542 70 28 minecraft:bamboo_sapling',
    'setblock 546 70 28 minecraft:bamboo',
    'setblock 546 71 28 minecraft:bamboo',
    'setblock 550 70 28 minecraft:bamboo',
];

const breakPhase = [
    'setblock 550 69 28 minecraft:air',
];

const verify = [
  'execute if block 522 70 28 minecraft:bamboo run say PASS_BAMBOO_ON_GRASS',
  'execute if block 526 70 28 minecraft:bamboo run say PASS_BAMBOO_ON_DIRT',
  'execute if block 530 70 28 minecraft:bamboo run say PASS_BAMBOO_ON_SAND',
  'execute if block 534 70 28 minecraft:bamboo run say PASS_BAMBOO_ON_GRAVEL',
  'execute if block 538 70 28 minecraft:bamboo_sapling run say PASS_BAMBOO_SAPLING_ON_DIRT',
  'execute if block 542 70 28 minecraft:bamboo_sapling run say PASS_BAMBOO_SAPLING_ON_SAND',
  'execute if block 546 70 28 minecraft:bamboo run say PASS_BAMBOO_STACK_BASE',
  'execute if block 546 71 28 minecraft:bamboo run say PASS_BAMBOO_STACK_TOP',
  'execute unless block 550 70 28 minecraft:bamboo run say PASS_SUPPORT_REMOVAL_BREAK',
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
      console.log('\n=== BAMBOO & BAMBOO SAPLING DUAL-SERVER DIFFERENTIAL SUMMARY ===');
      const expected = [
        'PASS_BAMBOO_ON_GRASS',
        'PASS_BAMBOO_ON_DIRT',
        'PASS_BAMBOO_ON_SAND',
        'PASS_BAMBOO_ON_GRAVEL',
        'PASS_BAMBOO_SAPLING_ON_DIRT',
        'PASS_BAMBOO_SAPLING_ON_SAND',
        'PASS_BAMBOO_STACK_BASE',
        'PASS_BAMBOO_STACK_TOP',
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

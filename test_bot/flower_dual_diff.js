const mc = require('minecraft-protocol');

function buildSetup() {
  return [
    'tp @s 370 85 28',
    'kill @e[type=item,x=360,y=60,z=20,dx=30,dy=35,dz=15]',
    'fill 360 67 25 390 76 31 air',
    'fill 360 68 25 390 68 31 minecraft:stone',
    
    // Test 1: Dandelion on dirt
    'setblock 362 69 28 minecraft:dirt',
    
    // Test 2: Poppy on dirt
    'setblock 366 69 28 minecraft:dirt',
    
    // Test 3: Cornflower on dirt
    'setblock 370 69 28 minecraft:dirt',
    
    // Test 4: Pink petals on dirt
    'setblock 374 69 28 minecraft:dirt',
    
    // Test 5: Support removal
    'setblock 378 69 28 minecraft:dirt',
  ];
}

const placementPhase = [
    'setblock 362 70 28 minecraft:dandelion',
    'setblock 366 70 28 minecraft:poppy',
    'setblock 370 70 28 minecraft:cornflower',
    'setblock 374 70 28 minecraft:pink_petals[flower_amount=1,facing=north]',
    'setblock 378 70 28 minecraft:dandelion',
];

const breakPhase = [
    'setblock 378 69 28 minecraft:air',
];

const verify = [
  'execute if block 362 70 28 minecraft:dandelion run say PASS_DANDELION_ON_DIRT',
  'execute if block 366 70 28 minecraft:poppy run say PASS_POPPY_ON_DIRT',
  'execute if block 370 70 28 minecraft:cornflower run say PASS_CORNFLOWER_ON_DIRT',
  'execute if block 374 70 28 minecraft:pink_petals run say PASS_PINK_PETALS_ON_DIRT',
  'execute unless block 378 70 28 minecraft:dandelion run say PASS_SUPPORT_REMOVAL_BREAK',
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
      console.log('\n=== FLOWER DUAL-SERVER DIFFERENTIAL SUMMARY ===');
      const expected = [
        'PASS_DANDELION_ON_DIRT',
        'PASS_POPPY_ON_DIRT',
        'PASS_CORNFLOWER_ON_DIRT',
        'PASS_PINK_PETALS_ON_DIRT',
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

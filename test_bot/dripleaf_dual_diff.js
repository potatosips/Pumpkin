const mc = require('minecraft-protocol');

function buildSetup() {
  return [
    'tp @s 575 85 28',
    'kill @e[type=item,x=560,y=60,z=20,dx=40,dy=35,dz=15]',
    'fill 560 67 25 600 76 31 air',
    'fill 560 68 25 600 68 31 minecraft:stone',
    
    // Test 1: Big dripleaf on clay
    'setblock 562 69 28 minecraft:clay',
    
    // Test 2: Big dripleaf on moss
    'setblock 566 69 28 minecraft:moss_block',
    
    // Test 3: Big dripleaf on dirt
    'setblock 570 69 28 minecraft:dirt',
    
    // Test 4: Big dripleaf stem column
    'setblock 574 69 28 minecraft:clay',
    
    // Test 5: Small dripleaf on clay
    'setblock 578 69 28 minecraft:clay',
    
    // Test 6: Small dripleaf on moss
    'setblock 582 69 28 minecraft:moss_block',
    
    // Test 7: Support removal
    'setblock 586 69 28 minecraft:clay',
  ];
}

const placementPhase = [
    'setblock 562 70 28 minecraft:big_dripleaf[facing=north]',
    'setblock 566 70 28 minecraft:big_dripleaf[facing=north]',
    'setblock 570 70 28 minecraft:big_dripleaf[facing=north]',
    'setblock 574 70 28 minecraft:big_dripleaf[facing=north]',
    'setblock 574 71 28 minecraft:big_dripleaf[facing=north]',
    'setblock 578 70 28 minecraft:small_dripleaf[facing=north,half=lower]',
    'setblock 578 71 28 minecraft:small_dripleaf[facing=north,half=upper]',
    'setblock 582 70 28 minecraft:small_dripleaf[facing=north,half=lower]',
    'setblock 582 71 28 minecraft:small_dripleaf[facing=north,half=upper]',
    'setblock 586 70 28 minecraft:big_dripleaf[facing=north]',
];

const breakPhase = [
    'setblock 586 69 28 minecraft:air',
];

const verify = [
  'execute if block 562 70 28 minecraft:big_dripleaf run say PASS_BIG_DRIPLEAF_ON_CLAY',
  'execute if block 566 70 28 minecraft:big_dripleaf run say PASS_BIG_DRIPLEAF_ON_MOSS',
  'execute if block 570 70 28 minecraft:big_dripleaf run say PASS_BIG_DRIPLEAF_ON_DIRT',
  'execute if block 574 70 28 minecraft:big_dripleaf_stem run say PASS_BIG_DRIPLEAF_STEM_BASE',
  'execute if block 574 71 28 minecraft:big_dripleaf run say PASS_BIG_DRIPLEAF_STEM_TOP',
  'execute if block 578 70 28 minecraft:small_dripleaf run say PASS_SMALL_DRIPLEAF_LOWER',
  'execute if block 578 71 28 minecraft:small_dripleaf run say PASS_SMALL_DRIPLEAF_UPPER',
  'execute if block 582 70 28 minecraft:small_dripleaf run say PASS_SMALL_DRIPLEAF_ON_MOSS',
  'execute unless block 586 70 28 minecraft:big_dripleaf run say PASS_SUPPORT_REMOVAL_BREAK',
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
      console.log('\n=== DRIPLEAF DUAL-SERVER DIFFERENTIAL SUMMARY ===');
      const expected = [
        'PASS_BIG_DRIPLEAF_ON_CLAY',
        'PASS_BIG_DRIPLEAF_ON_MOSS',
        'PASS_BIG_DRIPLEAF_ON_DIRT',
        'PASS_BIG_DRIPLEAF_STEM_BASE',
        'PASS_BIG_DRIPLEAF_STEM_TOP',
        'PASS_SMALL_DRIPLEAF_LOWER',
        'PASS_SMALL_DRIPLEAF_UPPER',
        'PASS_SMALL_DRIPLEAF_ON_MOSS',
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

const mc = require('minecraft-protocol');

function buildSetup() {
  return [
    'tp @s 340 85 28',
    'kill @e[type=item,x=330,y=60,z=20,dx=30,dy=35,dz=15]',
    'fill 330 67 25 360 76 31 air',
    'fill 330 68 25 360 68 31 minecraft:stone',
    
    // Test 1: Short grass on dirt
    'setblock 332 69 28 minecraft:dirt',
    
    // Test 2: Fern on dirt
    'setblock 336 69 28 minecraft:dirt',
    
    // Test 3 & 4: Tall grass (2-block tall plant)
    'setblock 340 69 28 minecraft:dirt',
    
    // Test 5 & 6: Sunflower (2-block tall plant)
    'setblock 344 69 28 minecraft:dirt',
    
    // Test 7 & 8: Rose bush (2-block tall plant)
    'setblock 348 69 28 minecraft:dirt',
    
    // Test 9: Support removal
    'setblock 352 69 28 minecraft:dirt',
  ];
}

const placementPhase = [
    'setblock 332 70 28 minecraft:short_grass',
    'setblock 336 70 28 minecraft:fern',
    'setblock 340 70 28 minecraft:tall_grass[half=lower]',
    'setblock 340 71 28 minecraft:tall_grass[half=upper]',
    'setblock 344 70 28 minecraft:sunflower[half=lower]',
    'setblock 344 71 28 minecraft:sunflower[half=upper]',
    'setblock 348 70 28 minecraft:rose_bush[half=lower]',
    'setblock 348 71 28 minecraft:rose_bush[half=upper]',
    'setblock 352 70 28 minecraft:short_grass',
];

const breakPhase = [
    'setblock 352 69 28 minecraft:air',
];

const verify = [
  'execute if block 332 70 28 minecraft:short_grass run say PASS_SHORT_GRASS_ON_DIRT',
  'execute if block 336 70 28 minecraft:fern run say PASS_FERN_ON_DIRT',
  'execute if block 340 70 28 minecraft:tall_grass run say PASS_TALL_GRASS_LOWER',
  'execute if block 340 71 28 minecraft:tall_grass run say PASS_TALL_GRASS_UPPER',
  'execute if block 344 70 28 minecraft:sunflower run say PASS_SUNFLOWER_LOWER',
  'execute if block 344 71 28 minecraft:sunflower run say PASS_SUNFLOWER_UPPER',
  'execute if block 348 70 28 minecraft:rose_bush run say PASS_ROSE_BUSH_LOWER',
  'execute if block 348 71 28 minecraft:rose_bush run say PASS_ROSE_BUSH_UPPER',
  'execute unless block 352 70 28 minecraft:short_grass run say PASS_SUPPORT_REMOVAL_BREAK',
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
      console.log('\n=== PLANT DUAL-SERVER DIFFERENTIAL SUMMARY ===');
      const expected = [
        'PASS_SHORT_GRASS_ON_DIRT',
        'PASS_FERN_ON_DIRT',
        'PASS_TALL_GRASS_LOWER',
        'PASS_TALL_GRASS_UPPER',
        'PASS_SUNFLOWER_LOWER',
        'PASS_SUNFLOWER_UPPER',
        'PASS_ROSE_BUSH_LOWER',
        'PASS_ROSE_BUSH_UPPER',
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

const mc = require('minecraft-protocol');

function buildSetup() {
  return [
    'tp @s 205 85 28',
    'kill @e[type=item,x=200,y=60,z=20,dx=25,dy=35,dz=15]',
    'fill 200 67 25 220 75 31 air',
    
    // Stone foundation
    'fill 200 68 25 220 68 31 minecraft:stone',
    
    // Test 1: Single sea pickle on stone in water at (202, 70, 28)
    'setblock 202 69 28 minecraft:stone',
    'setblock 202 70 28 minecraft:water',
    
    // Test 2: Sea pickle on dirt in water at (206, 70, 28)
    'setblock 206 69 28 minecraft:dirt',
    'setblock 206 70 28 minecraft:water',
    
    // Test 3: Sea pickle with pickles=4 at (210, 70, 28)
    'setblock 210 69 28 minecraft:stone',
    'setblock 210 70 28 minecraft:water',
    
    // Test 4: Sea pickle NOT waterlogged (dry, on land) at (214, 70, 28)
    'setblock 214 69 28 minecraft:stone',
    
    // Test 5: Support removal test at (218, 70, 28)
    'setblock 218 69 28 minecraft:dirt',
    'setblock 218 70 28 minecraft:water',
  ];
}

const placementPhase = [
    // Now place sea pickles after foundations are set
    'setblock 202 70 28 minecraft:sea_pickle[pickles=1,waterlogged=true]',
    'setblock 206 70 28 minecraft:sea_pickle[pickles=1,waterlogged=true]',
    'setblock 210 70 28 minecraft:sea_pickle[pickles=4,waterlogged=true]',
    'setblock 214 70 28 minecraft:sea_pickle[pickles=1,waterlogged=false]',
    'setblock 218 70 28 minecraft:sea_pickle[pickles=1,waterlogged=true]',
];

const breakPhase = [
    // Break support for test 5
    'setblock 218 69 28 minecraft:air',
];

const verify = [
  'execute if block 202 70 28 minecraft:sea_pickle run say PASS_PICKLE_ON_STONE',
  'execute if block 206 70 28 minecraft:sea_pickle run say PASS_PICKLE_ON_DIRT',
  'execute if block 210 70 28 minecraft:sea_pickle run say PASS_PICKLE_COUNT_4',
  'execute if block 214 70 28 minecraft:sea_pickle run say PASS_PICKLE_DRY',
  'execute unless block 218 70 28 minecraft:sea_pickle run say PASS_SUPPORT_REMOVAL_BREAK',
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
      // Phase 1: foundations
      setup.forEach((command, index) => setTimeout(() => {
        client.write('chat_command', { command, timestamp: BigInt(Date.now()) });
      }, index * 150));

      // Phase 2: place sea pickles (after foundations settle)
      const placeStart = setup.length * 150 + 2000;
      placementPhase.forEach((command, index) => setTimeout(() => {
        client.write('chat_command', { command, timestamp: BigInt(Date.now()) });
      }, placeStart + index * 200));

      // Phase 3: break support
      const breakStart = placeStart + placementPhase.length * 200 + 2000;
      breakPhase.forEach((command, index) => setTimeout(() => {
        client.write('chat_command', { command, timestamp: BigInt(Date.now()) });
      }, breakStart + index * 200));

      // Phase 4: verify
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
      console.log('\n=== SEA PICKLE DUAL-SERVER DIFFERENTIAL SUMMARY ===');
      const expected = [
        'PASS_PICKLE_ON_STONE',
        'PASS_PICKLE_ON_DIRT',
        'PASS_PICKLE_COUNT_4',
        'PASS_PICKLE_DRY',
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

const mc = require('minecraft-protocol');

function buildSetup() {
  return [
    'tp @s 225 85 28',
    'kill @e[type=item,x=220,y=60,z=20,dx=25,dy=35,dz=15]',
    'fill 220 67 25 240 75 31 air',
    
    // Stone foundation
    'fill 220 68 25 240 68 31 minecraft:stone',
    
    // Test 1: Lily pad on water at (222, 70, 28)
    'setblock 222 69 28 minecraft:stone',
    'setblock 222 70 28 minecraft:water',
    
    // Test 2: Lily pad on water at (226, 70, 28) - different support
    'setblock 226 69 28 minecraft:dirt',
    'setblock 226 70 28 minecraft:water',
    
    // Test 3: Support removal - water drained from under lily pad at (230, 70, 28)
    'setblock 230 69 28 minecraft:stone',
    'setblock 230 70 28 minecraft:water',
    
    // Test 4: Frosted ice support at (234, 70, 28)
    'setblock 234 69 28 minecraft:stone',
    'setblock 234 70 28 minecraft:frosted_ice',
  ];
}

const placementPhase = [
    'setblock 222 71 28 minecraft:lily_pad',
    'setblock 226 71 28 minecraft:lily_pad',
    'setblock 230 71 28 minecraft:lily_pad',
    'setblock 234 71 28 minecraft:lily_pad',
];

const breakPhase = [
    // Drain water to test support removal
    'setblock 230 70 28 minecraft:air',
];

const verify = [
  'execute if block 222 71 28 minecraft:lily_pad run say PASS_LILYPAD_ON_WATER_STONE',
  'execute if block 226 71 28 minecraft:lily_pad run say PASS_LILYPAD_ON_WATER_DIRT',
  'execute unless block 230 71 28 minecraft:lily_pad run say PASS_LILYPAD_SUPPORT_LOSS',
  'execute if block 234 71 28 minecraft:lily_pad run say PASS_LILYPAD_ON_ICE',
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
      console.log('\n=== LILY PAD DUAL-SERVER DIFFERENTIAL SUMMARY ===');
      const expected = [
        'PASS_LILYPAD_ON_WATER_STONE',
        'PASS_LILYPAD_ON_WATER_DIRT',
        'PASS_LILYPAD_SUPPORT_LOSS',
        'PASS_LILYPAD_ON_ICE',
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

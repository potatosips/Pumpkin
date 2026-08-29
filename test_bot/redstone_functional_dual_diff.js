const mc = require('minecraft-protocol');

function buildSetup() {
  return [
    'tp @s 1420 85 28',
    'kill @e[type=item,x=1400,y=60,z=20,dx=55,dy=35,dz=15]',
    'fill 1400 67 25 1450 76 31 air',
    'fill 1400 68 25 1450 68 31 minecraft:stone',
    
    // Foundations
    'setblock 1402 69 28 minecraft:stone',
    'setblock 1406 69 28 minecraft:stone',
    'setblock 1410 69 28 minecraft:stone',
    'setblock 1414 69 28 minecraft:stone',
    'setblock 1418 69 28 minecraft:stone',
    'setblock 1422 69 28 minecraft:stone',
    'setblock 1426 69 28 minecraft:stone',
    'setblock 1430 69 28 minecraft:stone',
    'setblock 1434 69 28 minecraft:stone',
  ];
}

const placementPhase = [
    'setblock 1402 70 28 minecraft:dispenser',
    'setblock 1406 70 28 minecraft:dropper',
    'setblock 1410 70 28 minecraft:crafter',
    'setblock 1414 70 28 minecraft:hopper',
    'setblock 1418 70 28 minecraft:daylight_detector',
    'setblock 1422 70 28 minecraft:copper_bulb',
    'setblock 1426 70 28 minecraft:target',
    'setblock 1430 70 28 minecraft:note_block',
    'setblock 1434 70 28 minecraft:jukebox',
];

const verify = [
  'execute if block 1402 70 28 minecraft:dispenser run say PASS_DISPENSER',
  'execute if block 1406 70 28 minecraft:dropper run say PASS_DROPPER',
  'execute if block 1410 70 28 minecraft:crafter run say PASS_CRAFTER',
  'execute if block 1414 70 28 minecraft:hopper run say PASS_HOPPER',
  'execute if block 1418 70 28 minecraft:daylight_detector run say PASS_DAYLIGHT_DETECTOR',
  'execute if block 1422 70 28 minecraft:copper_bulb run say PASS_COPPER_BULB',
  'execute if block 1426 70 28 minecraft:target run say PASS_TARGET_BLOCK',
  'execute if block 1430 70 28 minecraft:note_block run say PASS_NOTE_BLOCK',
  'execute if block 1434 70 28 minecraft:jukebox run say PASS_JUKEBOX',
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

      const verifyStart = placeStart + placementPhase.length * 200 + 2000;
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
      console.log('\n=== REDSTONE FUNCTIONAL BLOCKS DUAL-SERVER DIFFERENTIAL SUMMARY ===');
      const expected = [
        'PASS_DISPENSER',
        'PASS_DROPPER',
        'PASS_CRAFTER',
        'PASS_HOPPER',
        'PASS_DAYLIGHT_DETECTOR',
        'PASS_COPPER_BULB',
        'PASS_TARGET_BLOCK',
        'PASS_NOTE_BLOCK',
        'PASS_JUKEBOX',
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

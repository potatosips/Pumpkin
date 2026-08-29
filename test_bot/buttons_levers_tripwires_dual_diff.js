const mc = require('minecraft-protocol');

function buildSetup() {
  return [
    'tp @s 1265 85 28',
    'kill @e[type=item,x=1250,y=60,z=20,dx=45,dy=35,dz=15]',
    'fill 1250 67 25 1295 76 31 air',
    'fill 1250 68 25 1295 68 31 minecraft:stone',
    
    // Foundations
    'setblock 1252 69 28 minecraft:stone',
    'setblock 1256 70 27 minecraft:stone',
    'setblock 1260 69 28 minecraft:stone',
    'setblock 1264 70 27 minecraft:stone',
    'setblock 1268 70 27 minecraft:stone',
    'setblock 1272 69 28 minecraft:stone',
    'setblock 1276 69 28 minecraft:stone',
  ];
}

const placementPhase = [
    'setblock 1252 70 28 minecraft:stone_button[face=floor]',
    'setblock 1256 70 28 minecraft:oak_button[face=wall,facing=south]',
    'setblock 1260 70 28 minecraft:lever[face=floor]',
    'setblock 1264 70 28 minecraft:lever[face=wall,facing=south]',
    'setblock 1268 70 28 minecraft:tripwire_hook[facing=south]',
    'setblock 1272 70 28 minecraft:tripwire',
    'setblock 1276 70 28 minecraft:lever[face=floor]',
];

const breakPhase = [
    'setblock 1276 69 28 minecraft:air',
];

const verify = [
  'execute if block 1252 70 28 minecraft:stone_button run say PASS_STONE_BUTTON_ON_FLOOR',
  'execute if block 1256 70 28 minecraft:oak_button run say PASS_OAK_BUTTON_ON_WALL',
  'execute if block 1260 70 28 minecraft:lever run say PASS_LEVER_ON_FLOOR',
  'execute if block 1264 70 28 minecraft:lever run say PASS_LEVER_ON_WALL',
  'execute if block 1268 70 28 minecraft:tripwire_hook run say PASS_TRIPWIRE_HOOK_ON_WALL',
  'execute if block 1272 70 28 minecraft:tripwire run say PASS_TRIPWIRE_ON_FLOOR',
  'execute unless block 1276 70 28 minecraft:lever run say PASS_LEVER_SUPPORT_REMOVAL_BREAK',
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
      console.log('\n=== BUTTONS, LEVERS & TRIPWIRES DUAL-SERVER DIFFERENTIAL SUMMARY ===');
      const expected = [
        'PASS_STONE_BUTTON_ON_FLOOR',
        'PASS_OAK_BUTTON_ON_WALL',
        'PASS_LEVER_ON_FLOOR',
        'PASS_LEVER_ON_WALL',
        'PASS_TRIPWIRE_HOOK_ON_WALL',
        'PASS_TRIPWIRE_ON_FLOOR',
        'PASS_LEVER_SUPPORT_REMOVAL_BREAK',
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

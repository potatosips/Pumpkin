const mc = require('minecraft-protocol');

function buildSetup() {
  return [
    'tp @s 1465 85 28',
    'kill @e[type=item,x=1450,y=60,z=20,dx=45,dy=35,dz=15]',
    'fill 1450 67 25 1495 76 31 air',
    'fill 1450 68 25 1495 68 31 minecraft:stone',
    
    // Foundations
    'setblock 1452 69 28 minecraft:stone',
    'setblock 1456 69 28 minecraft:stone',
    'setblock 1460 69 28 minecraft:stone',
    'setblock 1464 69 28 minecraft:stone',
    'setblock 1468 69 28 minecraft:stone',
    'setblock 1472 69 28 minecraft:stone',
    'setblock 1476 69 28 minecraft:stone',
    'setblock 1480 69 28 minecraft:stone',
  ];
}

const placementPhase = [
    'setblock 1452 70 28 minecraft:chest',
    'setblock 1456 70 28 minecraft:trapped_chest',
    'setblock 1460 70 28 minecraft:ender_chest',
    'setblock 1464 70 28 minecraft:barrel',
    'setblock 1468 70 28 minecraft:shulker_box',
    'setblock 1472 70 28 minecraft:white_shulker_box',
    'setblock 1476 70 28 minecraft:decorated_pot',
    'setblock 1480 70 28 minecraft:chiseled_bookshelf',
];

const verify = [
  'execute if block 1452 70 28 minecraft:chest run say PASS_CHEST',
  'execute if block 1456 70 28 minecraft:trapped_chest run say PASS_TRAPPED_CHEST',
  'execute if block 1460 70 28 minecraft:ender_chest run say PASS_ENDER_CHEST',
  'execute if block 1464 70 28 minecraft:barrel run say PASS_BARREL',
  'execute if block 1468 70 28 minecraft:shulker_box run say PASS_SHULKER_BOX',
  'execute if block 1472 70 28 minecraft:white_shulker_box run say PASS_WHITE_SHULKER_BOX',
  'execute if block 1476 70 28 minecraft:decorated_pot run say PASS_DECORATED_POT',
  'execute if block 1480 70 28 minecraft:chiseled_bookshelf run say PASS_CHISELED_BOOKSHELF',
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
      console.log('\n=== CONTAINERS, CHESTS & POTS DUAL-SERVER DIFFERENTIAL SUMMARY ===');
      const expected = [
        'PASS_CHEST',
        'PASS_TRAPPED_CHEST',
        'PASS_ENDER_CHEST',
        'PASS_BARREL',
        'PASS_SHULKER_BOX',
        'PASS_WHITE_SHULKER_BOX',
        'PASS_DECORATED_POT',
        'PASS_CHISELED_BOOKSHELF',
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

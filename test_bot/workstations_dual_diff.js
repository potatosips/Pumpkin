const mc = require('minecraft-protocol');

function buildSetup() {
  return [
    'tp @s 1480 85 28',
    'kill @e[type=item,x=1440,y=60,z=20,dx=75,dy=35,dz=15]',
    'fill 1445 67 25 1515 76 31 air',
    'fill 1445 68 25 1515 68 31 minecraft:stone',
    
    // Foundations
    'setblock 1452 69 28 minecraft:stone',
    'setblock 1456 69 28 minecraft:stone',
    'setblock 1460 69 28 minecraft:stone',
    'setblock 1464 69 28 minecraft:stone',
    'setblock 1468 69 28 minecraft:stone',
    'setblock 1472 69 28 minecraft:stone',
    'setblock 1476 69 28 minecraft:stone',
    'setblock 1480 69 28 minecraft:stone',
    'setblock 1484 69 28 minecraft:stone',
    'setblock 1488 69 28 minecraft:stone',
    'setblock 1492 69 28 minecraft:stone',
    'setblock 1496 69 28 minecraft:stone',
    'setblock 1500 69 28 minecraft:stone',
    'setblock 1504 69 28 minecraft:stone',
    'setblock 1508 69 28 minecraft:stone',
  ];
}

const placementPhase = [
    'setblock 1452 70 28 minecraft:crafting_table',
    'setblock 1456 70 28 minecraft:furnace',
    'setblock 1460 70 28 minecraft:blast_furnace',
    'setblock 1464 70 28 minecraft:smoker',
    'setblock 1468 70 28 minecraft:anvil',
    'setblock 1472 70 28 minecraft:chipped_anvil',
    'setblock 1476 70 28 minecraft:damaged_anvil',
    'setblock 1480 70 28 minecraft:grindstone',
    'setblock 1484 70 28 minecraft:stonecutter',
    'setblock 1488 70 28 minecraft:loom',
    'setblock 1492 70 28 minecraft:cartography_table',
    'setblock 1496 70 28 minecraft:smithing_table',
    'setblock 1500 70 28 minecraft:brewing_stand',
    'setblock 1504 70 28 minecraft:enchanting_table',
    'setblock 1508 70 28 minecraft:beacon',
];

const verify = [
  'execute if block 1452 70 28 minecraft:crafting_table run say PASS_CRAFTING_TABLE',
  'execute if block 1456 70 28 minecraft:furnace run say PASS_FURNACE',
  'execute if block 1460 70 28 minecraft:blast_furnace run say PASS_BLAST_FURNACE',
  'execute if block 1464 70 28 minecraft:smoker run say PASS_SMOKER',
  'execute if block 1468 70 28 minecraft:anvil run say PASS_ANVIL',
  'execute if block 1472 70 28 minecraft:chipped_anvil run say PASS_CHIPPED_ANVIL',
  'execute if block 1476 70 28 minecraft:damaged_anvil run say PASS_DAMAGED_ANVIL',
  'execute if block 1480 70 28 minecraft:grindstone run say PASS_GRINDSTONE',
  'execute if block 1484 70 28 minecraft:stonecutter run say PASS_STONECUTTER',
  'execute if block 1488 70 28 minecraft:loom run say PASS_LOOM',
  'execute if block 1492 70 28 minecraft:cartography_table run say PASS_CARTOGRAPHY_TABLE',
  'execute if block 1496 70 28 minecraft:smithing_table run say PASS_SMITHING_TABLE',
  'execute if block 1500 70 28 minecraft:brewing_stand run say PASS_BREWING_STAND',
  'execute if block 1504 70 28 minecraft:enchanting_table run say PASS_ENCHANTING_TABLE',
  'execute if block 1508 70 28 minecraft:beacon run say PASS_BEACON',
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
      console.log('\n=== WORKSTATIONS & UTILITY BLOCKS DUAL-SERVER DIFFERENTIAL SUMMARY ===');
      const expected = [
        'PASS_CRAFTING_TABLE',
        'PASS_FURNACE',
        'PASS_BLAST_FURNACE',
        'PASS_SMOKER',
        'PASS_ANVIL',
        'PASS_CHIPPED_ANVIL',
        'PASS_DAMAGED_ANVIL',
        'PASS_GRINDSTONE',
        'PASS_STONECUTTER',
        'PASS_LOOM',
        'PASS_CARTOGRAPHY_TABLE',
        'PASS_SMITHING_TABLE',
        'PASS_BREWING_STAND',
        'PASS_ENCHANTING_TABLE',
        'PASS_BEACON',
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

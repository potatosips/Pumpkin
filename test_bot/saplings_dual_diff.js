const mc = require('minecraft-protocol');

function buildSetup() {
  return [
    'tp @s 615 85 28',
    'kill @e[type=item,x=600,y=60,z=20,dx=45,dy=35,dz=15]',
    'fill 600 67 25 645 76 31 air',
    'fill 600 68 25 645 68 31 minecraft:stone',
    
    // Test 1: Oak sapling on grass
    'setblock 602 69 28 minecraft:grass_block',
    
    // Test 2: Spruce sapling on podzol
    'setblock 606 69 28 minecraft:podzol',
    
    // Test 3: Birch sapling on dirt
    'setblock 610 69 28 minecraft:dirt',
    
    // Test 4: Jungle sapling on dirt
    'setblock 614 69 28 minecraft:dirt',
    
    // Test 5: Acacia sapling on grass
    'setblock 618 69 28 minecraft:grass_block',
    
    // Test 6: Dark oak sapling on dirt
    'setblock 622 69 28 minecraft:dirt',
    
    // Test 7: Cherry sapling on moss
    'setblock 626 69 28 minecraft:moss_block',
    
    // Test 8: Pale oak sapling on dirt
    'setblock 630 69 28 minecraft:dirt',
    
    // Test 9: Support removal
    'setblock 634 69 28 minecraft:dirt',
  ];
}

const placementPhase = [
    'setblock 602 70 28 minecraft:oak_sapling',
    'setblock 606 70 28 minecraft:spruce_sapling',
    'setblock 610 70 28 minecraft:birch_sapling',
    'setblock 614 70 28 minecraft:jungle_sapling',
    'setblock 618 70 28 minecraft:acacia_sapling',
    'setblock 622 70 28 minecraft:dark_oak_sapling',
    'setblock 626 70 28 minecraft:cherry_sapling',
    'setblock 630 70 28 minecraft:pale_oak_sapling',
    'setblock 634 70 28 minecraft:oak_sapling',
];

const breakPhase = [
    'setblock 634 69 28 minecraft:air',
];

const verify = [
  'execute if block 602 70 28 minecraft:oak_sapling run say PASS_OAK_SAPLING_ON_GRASS',
  'execute if block 606 70 28 minecraft:spruce_sapling run say PASS_SPRUCE_SAPLING_ON_PODZOL',
  'execute if block 610 70 28 minecraft:birch_sapling run say PASS_BIRCH_SAPLING_ON_DIRT',
  'execute if block 614 70 28 minecraft:jungle_sapling run say PASS_JUNGLE_SAPLING_ON_DIRT',
  'execute if block 618 70 28 minecraft:acacia_sapling run say PASS_ACACIA_SAPLING_ON_GRASS',
  'execute if block 622 70 28 minecraft:dark_oak_sapling run say PASS_DARK_OAK_SAPLING_ON_DIRT',
  'execute if block 626 70 28 minecraft:cherry_sapling run say PASS_CHERRY_SAPLING_ON_MOSS',
  'execute if block 630 70 28 minecraft:pale_oak_sapling run say PASS_PALE_OAK_SAPLING_ON_DIRT',
  'execute unless block 634 70 28 minecraft:oak_sapling run say PASS_SUPPORT_REMOVAL_BREAK',
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
      console.log('\n=== SAPLINGS DUAL-SERVER DIFFERENTIAL SUMMARY ===');
      const expected = [
        'PASS_OAK_SAPLING_ON_GRASS',
        'PASS_SPRUCE_SAPLING_ON_PODZOL',
        'PASS_BIRCH_SAPLING_ON_DIRT',
        'PASS_JUNGLE_SAPLING_ON_DIRT',
        'PASS_ACACIA_SAPLING_ON_GRASS',
        'PASS_DARK_OAK_SAPLING_ON_DIRT',
        'PASS_CHERRY_SAPLING_ON_MOSS',
        'PASS_PALE_OAK_SAPLING_ON_DIRT',
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

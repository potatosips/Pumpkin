const mc = require('minecraft-protocol');

function buildSetup() {
  return [
    'tp @s 665 85 28',
    'kill @e[type=item,x=650,y=60,z=20,dx=45,dy=35,dz=15]',
    'fill 650 67 25 695 76 31 air',
    'fill 650 68 25 695 68 31 minecraft:stone',
    
    // Farmland foundations
    'setblock 652 69 28 minecraft:farmland',
    'setblock 656 69 28 minecraft:farmland',
    'setblock 660 69 28 minecraft:farmland',
    'setblock 664 69 28 minecraft:farmland',
    'setblock 668 69 28 minecraft:farmland',
    'setblock 672 69 28 minecraft:soul_sand',
    'setblock 676 69 28 minecraft:farmland',
    'setblock 680 69 28 minecraft:farmland',
    'setblock 684 69 28 minecraft:farmland',
  ];
}

const placementPhase = [
    'setblock 652 70 28 minecraft:wheat',
    'setblock 656 70 28 minecraft:carrots',
    'setblock 660 70 28 minecraft:potatoes',
    'setblock 664 70 28 minecraft:beetroots',
    'setblock 668 70 28 minecraft:torchflower_crop',
    'setblock 672 70 28 minecraft:nether_wart',
    'setblock 676 70 28 minecraft:pumpkin_stem',
    'setblock 680 70 28 minecraft:melon_stem',
    'setblock 684 70 28 minecraft:wheat',
];

const breakPhase = [
    'setblock 684 69 28 minecraft:air',
];

const verify = [
  'execute if block 652 70 28 minecraft:wheat run say PASS_WHEAT_ON_FARMLAND',
  'execute if block 656 70 28 minecraft:carrots run say PASS_CARROTS_ON_FARMLAND',
  'execute if block 660 70 28 minecraft:potatoes run say PASS_POTATOES_ON_FARMLAND',
  'execute if block 664 70 28 minecraft:beetroots run say PASS_BEETROOTS_ON_FARMLAND',
  'execute if block 668 70 28 minecraft:torchflower_crop run say PASS_TORCHFLOWER_ON_FARMLAND',
  'execute if block 672 70 28 minecraft:nether_wart run say PASS_NETHER_WART_ON_SOUL_SAND',
  'execute if block 676 70 28 minecraft:pumpkin_stem run say PASS_PUMPKIN_STEM_ON_FARMLAND',
  'execute if block 680 70 28 minecraft:melon_stem run say PASS_MELON_STEM_ON_FARMLAND',
  'execute unless block 684 70 28 minecraft:wheat run say PASS_SUPPORT_REMOVAL_BREAK',
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
      console.log('\n=== CROPS DUAL-SERVER DIFFERENTIAL SUMMARY ===');
      const expected = [
        'PASS_WHEAT_ON_FARMLAND',
        'PASS_CARROTS_ON_FARMLAND',
        'PASS_POTATOES_ON_FARMLAND',
        'PASS_BEETROOTS_ON_FARMLAND',
        'PASS_TORCHFLOWER_ON_FARMLAND',
        'PASS_NETHER_WART_ON_SOUL_SAND',
        'PASS_PUMPKIN_STEM_ON_FARMLAND',
        'PASS_MELON_STEM_ON_FARMLAND',
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

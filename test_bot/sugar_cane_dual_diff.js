const mc = require('minecraft-protocol');

function buildSetup() {
  return [
    'tp @s 185 85 28',
    'kill @e[type=item,x=175,y=60,z=20,dx=25,dy=35,dz=15]',
    'fill 176 67 25 196 75 31 air',
    
    // Support foundations at Y=68:
    'fill 176 68 25 196 68 31 minecraft:stone',
    
    // Base 1: Sand at (178, 69, 28) with Water at (178, 69, 27) and Sugar Cane at (178, 70, 28)
    'setblock 178 69 27 minecraft:water',
    'setblock 178 69 28 minecraft:sand',
    'setblock 178 70 28 minecraft:sugar_cane',
    
    // Base 2: Dirt at (182, 69, 28) with Water at (182, 69, 27) and Sugar Cane at (182, 70, 28)
    'setblock 182 69 27 minecraft:water',
    'setblock 182 69 28 minecraft:dirt',
    'setblock 182 70 28 minecraft:sugar_cane',
    
    // Base 3: Grass Block at (186, 69, 28) with Water at (186, 69, 27) and Sugar Cane at (186, 70, 28)
    'setblock 186 69 27 minecraft:water',
    'setblock 186 69 28 minecraft:grass_block',
    'setblock 186 70 28 minecraft:sugar_cane',
    
    // Base 4: Multi-block spire (3 sugar canes stacked) at (190, 69, 28) with Water at (190, 69, 27)
    'setblock 190 69 27 minecraft:water',
    'setblock 190 69 28 minecraft:sand',
    'setblock 190 70 28 minecraft:sugar_cane',
    'setblock 190 71 28 minecraft:sugar_cane',
    'setblock 190 72 28 minecraft:sugar_cane',
    
    // Base 5: Support removal test at (194, 69, 28)
    'setblock 194 69 27 minecraft:water',
    'setblock 194 69 28 minecraft:sand',
    'setblock 194 70 28 minecraft:sugar_cane',
    // Break the sand beneath the sugar cane:
    'setblock 194 69 28 minecraft:air',
  ];
}

const setup = buildSetup();
const verify = [
  'execute if block 178 70 28 minecraft:sugar_cane run say PASS_SUGAR_CANE_ON_SAND_WATER',
  'execute if block 182 70 28 minecraft:sugar_cane run say PASS_SUGAR_CANE_ON_DIRT_WATER',
  'execute if block 186 70 28 minecraft:sugar_cane run say PASS_SUGAR_CANE_ON_GRASS_WATER',
  'execute if block 190 70 28 minecraft:sugar_cane run say PASS_SPIRE_BASE',
  'execute if block 190 71 28 minecraft:sugar_cane run say PASS_SPIRE_MID',
  'execute if block 190 72 28 minecraft:sugar_cane run say PASS_SPIRE_TOP',
  // Removed support should cause sugar cane at 194 70 28 to break:
  'execute unless block 194 70 28 minecraft:sugar_cane run say PASS_SUPPORT_REMOVAL_BREAK',
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
      setup.forEach((command, index) => setTimeout(() => {
        client.write('chat_command', { command, timestamp: BigInt(Date.now()) });
      }, index * 100));

      const verifyStart = setup.length * 100 + 3000;
      verify.forEach((command, index) => setTimeout(() => {
        client.write('chat_command', { command, timestamp: BigInt(Date.now()) });
      }, verifyStart + index * 150));

      setTimeout(() => client.end(), verifyStart + verify.length * 150 + 1000);
    }, 500);
  });

  client.on('system_chat', packet => handleMsg(name, packet.content));
  client.on('profileless_chat', packet => handleMsg(name, packet.message));
  client.on('disguised_chat', packet => handleMsg(name, packet.message));
  client.on('player_chat', packet => handleMsg(name, packet.unsignedContent || packet.plainMessage || packet.signedChatContent || packet));

  client.on('error', error => console.error(`[${name}] ERROR ${error.message}`));
  client.on('end', () => {
    if (++finished === 2) {
      console.log('\n=== SUGAR CANE DUAL-SERVER DIFFERENTIAL SUMMARY ===');
      const expected = [
        'PASS_SUGAR_CANE_ON_SAND_WATER',
        'PASS_SUGAR_CANE_ON_DIRT_WATER',
        'PASS_SUGAR_CANE_ON_GRASS_WATER',
        'PASS_SPIRE_BASE',
        'PASS_SPIRE_MID',
        'PASS_SPIRE_TOP',
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

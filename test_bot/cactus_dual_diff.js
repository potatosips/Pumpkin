const mc = require('minecraft-protocol');

function buildSetup() {
  return [
    'tp @s 185 85 28',
    'kill @e[type=item,x=175,y=60,z=20,dx=25,dy=35,dz=15]',
    'fill 176 67 26 196 75 30 air',
    
    // Support foundations at Y=68 so sand doesn't fall:
    'setblock 178 68 28 minecraft:stone',
    'setblock 182 68 28 minecraft:stone',
    'setblock 186 68 28 minecraft:stone',
    'setblock 190 68 28 minecraft:stone',
    'setblock 194 68 28 minecraft:stone',
    
    // Base 1: Sand on stone foundation with Cactus:
    'setblock 178 69 28 minecraft:sand',
    'setblock 178 70 28 minecraft:cactus',
    
    // Base 2: Red Sand on stone foundation with Cactus:
    'setblock 182 69 28 minecraft:red_sand',
    'setblock 182 70 28 minecraft:cactus',
    
    // Base 3: Multi-block spire (3 cacti stacked on sand):
    'setblock 186 69 28 minecraft:sand',
    'setblock 186 70 28 minecraft:cactus',
    'setblock 186 71 28 minecraft:cactus',
    'setblock 186 72 28 minecraft:cactus',
    
    // Base 4: Horizontal solid neighbor placement at (190, 70, 28) on sand at 69
    'setblock 190 69 28 minecraft:sand',
    'setblock 190 70 28 minecraft:cactus',
    // Place solid stone directly adjacent horizontally at (190, 70, 29) to trigger break:
    'setblock 190 70 29 minecraft:stone',
    
    // Base 5: Support removal at (194, 70, 28) on sand at 69
    'setblock 194 69 28 minecraft:sand',
    'setblock 194 70 28 minecraft:cactus',
    // Break the sand to trigger cactus break:
    'setblock 194 69 28 minecraft:air',
  ];
}

const setup = buildSetup();
const verify = [
  'execute if block 178 70 28 minecraft:cactus run say PASS_CACTUS_ON_SAND',
  'execute if block 182 70 28 minecraft:cactus run say PASS_CACTUS_ON_RED_SAND',
  'execute if block 186 70 28 minecraft:cactus run say PASS_SPIRE_BASE',
  'execute if block 186 71 28 minecraft:cactus run say PASS_SPIRE_MID',
  'execute if block 186 72 28 minecraft:cactus run say PASS_SPIRE_TOP',
  // Adjacent stone should have broken the cactus at 190 70 28:
  'execute unless block 190 70 28 minecraft:cactus run say PASS_NEIGHBOR_BREAK',
  // Removed support should have broken the cactus at 194 70 28:
  'execute unless block 194 70 28 minecraft:cactus run say PASS_SUPPORT_LOSS_BREAK',
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
      console.log('\n=== CACTUS DUAL-SERVER DIFFERENTIAL SUMMARY ===');
      const expected = [
        'PASS_CACTUS_ON_SAND',
        'PASS_CACTUS_ON_RED_SAND',
        'PASS_SPIRE_BASE',
        'PASS_SPIRE_MID',
        'PASS_SPIRE_TOP',
        'PASS_NEIGHBOR_BREAK',
        'PASS_SUPPORT_LOSS_BREAK',
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

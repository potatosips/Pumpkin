const mc = require('minecraft-protocol');

function buildSetup() {
  return [
    'tp @s 460 85 28',
    'kill @e[type=item,x=450,y=60,z=20,dx=30,dy=35,dz=15]',
    'fill 450 67 25 480 76 31 air',
    'fill 450 68 25 480 68 31 minecraft:stone',
    
    // Test 1: Dead bush on sand
    'setblock 452 69 28 minecraft:sand',
    
    // Test 2: Dead bush on red sand
    'setblock 456 69 28 minecraft:red_sand',
    
    // Test 3: Dead bush on terracotta
    'setblock 460 69 28 minecraft:terracotta',
    
    // Test 4: Dead bush on dirt
    'setblock 464 69 28 minecraft:dirt',
    
    // Test 5: Support removal
    'setblock 468 69 28 minecraft:sand',
  ];
}

const placementPhase = [
    'setblock 452 70 28 minecraft:dead_bush',
    'setblock 456 70 28 minecraft:dead_bush',
    'setblock 460 70 28 minecraft:dead_bush',
    'setblock 464 70 28 minecraft:dead_bush',
    'setblock 468 70 28 minecraft:dead_bush',
];

const breakPhase = [
    'setblock 468 69 28 minecraft:air',
];

const verify = [
  'execute if block 452 70 28 minecraft:dead_bush run say PASS_DEAD_BUSH_ON_SAND',
  'execute if block 456 70 28 minecraft:dead_bush run say PASS_DEAD_BUSH_ON_RED_SAND',
  'execute if block 460 70 28 minecraft:dead_bush run say PASS_DEAD_BUSH_ON_TERRACOTTA',
  'execute if block 464 70 28 minecraft:dead_bush run say PASS_DEAD_BUSH_ON_DIRT',
  'execute unless block 468 70 28 minecraft:dead_bush run say PASS_SUPPORT_REMOVAL_BREAK',
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
      console.log('\n=== DRY VEGETATION & LEAF LITTER DUAL-SERVER DIFFERENTIAL SUMMARY ===');
      const expected = [
        'PASS_DEAD_BUSH_ON_SAND',
        'PASS_DEAD_BUSH_ON_RED_SAND',
        'PASS_DEAD_BUSH_ON_TERRACOTTA',
        'PASS_DEAD_BUSH_ON_DIRT',
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

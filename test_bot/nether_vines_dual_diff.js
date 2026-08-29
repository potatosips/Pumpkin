const mc = require('minecraft-protocol');

function buildSetup() {
  return [
    'tp @s 285 85 28',
    'kill @e[type=item,x=280,y=60,z=20,dx=25,dy=35,dz=15]',
    'fill 280 67 25 300 80 31 air',
    'fill 280 68 25 300 68 31 minecraft:netherrack',
    'fill 280 75 25 300 75 31 minecraft:netherrack',
    
    // Test 1: Twisting vines (grows upward) on netherrack floor
    'setblock 282 69 28 minecraft:twisting_vines',
    
    // Test 2: Twisting vines stacked (plant + tip)
    'setblock 286 69 28 minecraft:twisting_vines_plant',
    'setblock 286 70 28 minecraft:twisting_vines',
    
    // Test 3: Weeping vines (grows downward) from netherrack ceiling
    'setblock 290 74 28 minecraft:weeping_vines',
    
    // Test 4: Weeping vines stacked (plant + tip)
    'setblock 294 74 28 minecraft:weeping_vines_plant',
    'setblock 294 73 28 minecraft:weeping_vines',
    
    // Test 5: Twisting vines support removal
    'setblock 298 69 28 minecraft:twisting_vines',
  ];
}

const breakPhase = [
    'setblock 298 68 28 minecraft:air',
];

const verify = [
  'execute if block 282 69 28 minecraft:twisting_vines run say PASS_TWISTING_ON_FLOOR',
  'execute if block 286 69 28 minecraft:twisting_vines_plant run say PASS_TWISTING_STACK_BASE',
  'execute if block 286 70 28 minecraft:twisting_vines run say PASS_TWISTING_STACK_TOP',
  'execute if block 290 74 28 minecraft:weeping_vines run say PASS_WEEPING_ON_CEILING',
  'execute if block 294 74 28 minecraft:weeping_vines_plant run say PASS_WEEPING_STACK_BASE',
  'execute if block 294 73 28 minecraft:weeping_vines run say PASS_WEEPING_STACK_TIP',
  'execute unless block 298 69 28 minecraft:twisting_vines run say PASS_TWISTING_SUPPORT_REMOVAL',
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

      const breakStart = setup.length * 150 + 3000;
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
      console.log('\n=== NETHER VINES DUAL-SERVER DIFFERENTIAL SUMMARY ===');
      const expected = [
        'PASS_TWISTING_ON_FLOOR',
        'PASS_TWISTING_STACK_BASE',
        'PASS_TWISTING_STACK_TOP',
        'PASS_WEEPING_ON_CEILING',
        'PASS_WEEPING_STACK_BASE',
        'PASS_WEEPING_STACK_TIP',
        'PASS_TWISTING_SUPPORT_REMOVAL',
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

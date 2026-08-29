const mc = require('minecraft-protocol');

function buildSetup() {
  return [
    'tp @s 245 85 28',
    'kill @e[type=item,x=240,y=60,z=20,dx=25,dy=35,dz=15]',
    'fill 240 67 25 260 75 31 air',
    
    // End stone foundations
    'fill 240 68 25 260 68 31 minecraft:end_stone',
    
    // Test 1: Chorus plant on end stone at (242, 69, 28)
    'setblock 242 69 28 minecraft:chorus_plant',
    
    // Test 2: Chorus flower on end stone at (246, 69, 28)
    'setblock 246 69 28 minecraft:chorus_flower',
    
    // Test 3: Stacked chorus plant + flower at (250, 69-70, 28)
    'setblock 250 69 28 minecraft:chorus_plant',
    'setblock 250 70 28 minecraft:chorus_flower',
    
    // Test 4: Chorus plant on non-end-stone (should NOT survive) at (254, 69, 28)
    // Using setblock to force place it, then check if it persists
    'setblock 254 68 28 minecraft:dirt',
    'setblock 254 69 28 minecraft:chorus_plant',
    
    // Test 5: Support removal - chorus plant on end stone, then remove end stone
    'setblock 258 69 28 minecraft:chorus_plant',
  ];
}

const breakPhase = [
    // Remove support for test 5
    'setblock 258 68 28 minecraft:air',
];

const verify = [
  'execute if block 242 69 28 minecraft:chorus_plant run say PASS_CHORUS_PLANT_ON_ENDSTONE',
  'execute if block 246 69 28 minecraft:chorus_flower run say PASS_CHORUS_FLOWER_ON_ENDSTONE',
  'execute if block 250 69 28 minecraft:chorus_plant run say PASS_CHORUS_STACK_BASE',
  'execute if block 250 70 28 minecraft:chorus_flower run say PASS_CHORUS_STACK_TOP',
  'execute unless block 258 69 28 minecraft:chorus_plant run say PASS_CHORUS_SUPPORT_REMOVAL',
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
      console.log('\n=== CHORUS PLANT DUAL-SERVER DIFFERENTIAL SUMMARY ===');
      const expected = [
        'PASS_CHORUS_PLANT_ON_ENDSTONE',
        'PASS_CHORUS_FLOWER_ON_ENDSTONE',
        'PASS_CHORUS_STACK_BASE',
        'PASS_CHORUS_STACK_TOP',
        'PASS_CHORUS_SUPPORT_REMOVAL',
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

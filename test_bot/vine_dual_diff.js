const mc = require('minecraft-protocol');

function buildSetup() {
  return [
    'tp @s 265 85 28',
    'kill @e[type=item,x=260,y=60,z=20,dx=25,dy=35,dz=15]',
    'fill 260 67 25 280 75 31 air',
    
    // Stone walls for vine attachment
    'fill 260 68 25 280 68 31 minecraft:stone',
    
    // Test 1: Vine on north face of stone wall at (262, 70, 28)
    'setblock 262 70 27 minecraft:stone',
    'setblock 262 70 28 minecraft:vine[north=true]',
    
    // Test 2: Vine on south face at (266, 70, 28)
    'setblock 266 70 29 minecraft:stone',
    'setblock 266 70 28 minecraft:vine[south=true]',
    
    // Test 3: Vine on east face at (270, 70, 28)
    'setblock 271 70 28 minecraft:stone',
    'setblock 270 70 28 minecraft:vine[east=true]',
    
    // Test 4: Vine with up=true attached to ceiling at (274, 70, 28)
    'setblock 274 71 28 minecraft:stone',
    'setblock 274 70 28 minecraft:vine[up=true]',
    
    // Test 5: Support removal - vine on wall, remove wall
    'setblock 278 70 27 minecraft:stone',
    'setblock 278 70 28 minecraft:vine[north=true]',
  ];
}

const breakPhase = [
    'setblock 278 70 27 minecraft:air',
];

const verify = [
  'execute if block 262 70 28 minecraft:vine run say PASS_VINE_NORTH',
  'execute if block 266 70 28 minecraft:vine run say PASS_VINE_SOUTH',
  'execute if block 270 70 28 minecraft:vine run say PASS_VINE_EAST',
  'execute if block 274 70 28 minecraft:vine run say PASS_VINE_UP',
  'execute unless block 278 70 28 minecraft:vine run say PASS_VINE_SUPPORT_REMOVAL',
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
      console.log('\n=== VINE DUAL-SERVER DIFFERENTIAL SUMMARY ===');
      const expected = [
        'PASS_VINE_NORTH',
        'PASS_VINE_SOUTH',
        'PASS_VINE_EAST',
        'PASS_VINE_UP',
        'PASS_VINE_SUPPORT_REMOVAL',
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

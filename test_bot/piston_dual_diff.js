const mc = require('minecraft-protocol');

function buildSetup() {
  return [
    'tp @s 1065 85 28',
    'kill @e[type=item,x=1050,y=60,z=20,dx=45,dy=35,dz=15]',
    'fill 1050 67 25 1095 76 31 air',
    'fill 1050 68 25 1095 68 31 minecraft:stone',
    
    // Foundations
    'setblock 1052 69 28 minecraft:stone',
    'setblock 1056 69 28 minecraft:stone',
    'setblock 1060 69 28 minecraft:stone',
    'setblock 1064 69 28 minecraft:stone',
    'setblock 1068 69 28 minecraft:stone',
    'setblock 1072 69 28 minecraft:stone',
  ];
}

const placementPhase = [
    'setblock 1052 70 28 minecraft:piston[facing=up,extended=false]',
    'setblock 1056 70 28 minecraft:sticky_piston[facing=up,extended=false]',
    'setblock 1060 70 28 minecraft:piston[facing=north,extended=false]',
    'setblock 1064 70 28 minecraft:sticky_piston[facing=east,extended=false]',
    
    'setblock 1068 70 28 minecraft:piston[facing=up,extended=true]',
    'setblock 1068 71 28 minecraft:piston_head[facing=up,type=normal]',
    
    'setblock 1072 70 28 minecraft:sticky_piston[facing=up,extended=true]',
    'setblock 1072 71 28 minecraft:piston_head[facing=up,type=sticky]',
];

const breakPhase = [
    'setblock 1068 70 28 minecraft:air',
];

const verify = [
  'execute if block 1052 70 28 minecraft:piston run say PASS_PISTON_UP_PLACEMENT',
  'execute if block 1056 70 28 minecraft:sticky_piston run say PASS_STICKY_PISTON_UP_PLACEMENT',
  'execute if block 1060 70 28 minecraft:piston run say PASS_PISTON_FACING_NORTH',
  'execute if block 1064 70 28 minecraft:sticky_piston run say PASS_STICKY_PISTON_FACING_EAST',
  'execute if block 1072 71 28 minecraft:piston_head run say PASS_STICKY_PISTON_HEAD',
  'execute unless block 1068 71 28 minecraft:piston_head run say PASS_PISTON_BREAK_HEAD_DROP',
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
      console.log('\n=== PISTONS & PISTON HEADS DUAL-SERVER DIFFERENTIAL SUMMARY ===');
      const expected = [
        'PASS_PISTON_UP_PLACEMENT',
        'PASS_STICKY_PISTON_UP_PLACEMENT',
        'PASS_PISTON_FACING_NORTH',
        'PASS_STICKY_PISTON_FACING_EAST',
        'PASS_STICKY_PISTON_HEAD',
        'PASS_PISTON_BREAK_HEAD_DROP',
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

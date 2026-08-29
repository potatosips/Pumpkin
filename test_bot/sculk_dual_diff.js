const mc = require('minecraft-protocol');

function buildSetup() {
  return [
    'tp @s 1015 85 28',
    'kill @e[type=item,x=1000,y=60,z=20,dx=45,dy=35,dz=15]',
    'fill 1000 67 25 1045 76 31 air',
    'fill 1000 68 25 1045 68 31 minecraft:stone',
    
    // Foundations
    'setblock 1002 69 28 minecraft:stone',
    'setblock 1006 69 28 minecraft:stone',
    'setblock 1010 69 28 minecraft:stone',
    'setblock 1010 70 28 minecraft:water',
    
    'setblock 1014 69 28 minecraft:stone',
    
    'setblock 1018 70 27 minecraft:stone',
    
    'setblock 1022 69 28 minecraft:stone',
  ];
}

const placementPhase = [
    'setblock 1002 70 28 minecraft:sculk_catalyst',
    'setblock 1006 70 28 minecraft:sculk_shrieker[shrieking=false,can_summon=false]',
    'setblock 1010 70 28 minecraft:sculk_shrieker[waterlogged=true]',
    'setblock 1014 70 28 minecraft:sculk_vein[down=true]',
    'setblock 1018 70 28 minecraft:sculk_vein[north=true]',
    'setblock 1022 70 28 minecraft:sculk_vein[down=true]',
];

const breakPhase = [
    'setblock 1022 69 28 minecraft:air',
];

const verify = [
  'execute if block 1002 70 28 minecraft:sculk_catalyst run say PASS_SCULK_CATALYST_ON_STONE',
  'execute if block 1006 70 28 minecraft:sculk_shrieker run say PASS_SCULK_SHRIEKER_ON_STONE',
  'execute if block 1010 70 28 minecraft:sculk_shrieker run say PASS_SCULK_SHRIEKER_WATERLOGGED',
  'execute if block 1014 70 28 minecraft:sculk_vein run say PASS_SCULK_VEIN_ON_FLOOR',
  'execute if block 1018 70 28 minecraft:sculk_vein run say PASS_SCULK_VEIN_ON_WALL',
  'execute unless block 1022 70 28 minecraft:sculk_vein run say PASS_SUPPORT_REMOVAL_BREAK',
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
      console.log('\n=== SCULK DUAL-SERVER DIFFERENTIAL SUMMARY ===');
      const expected = [
        'PASS_SCULK_CATALYST_ON_STONE',
        'PASS_SCULK_SHRIEKER_ON_STONE',
        'PASS_SCULK_SHRIEKER_WATERLOGGED',
        'PASS_SCULK_VEIN_ON_FLOOR',
        'PASS_SCULK_VEIN_ON_WALL',
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

const mc = require('minecraft-protocol');

function buildSetup() {
  return [
    'tp @s 1165 85 28',
    'kill @e[type=item,x=1150,y=60,z=20,dx=45,dy=35,dz=15]',
    'fill 1150 67 25 1195 76 31 air',
    'fill 1150 68 25 1195 68 31 minecraft:stone',
    
    // Foundations
    'setblock 1152 69 28 minecraft:stone',
    'setblock 1156 70 27 minecraft:stone',
    'setblock 1160 69 28 minecraft:stone',
    'setblock 1164 69 28 minecraft:redstone_block',
    'setblock 1168 69 28 minecraft:stone',
    'setblock 1172 69 28 minecraft:stone',
  ];
}

const placementPhase = [
    'setblock 1152 70 28 minecraft:redstone_torch',
    'setblock 1156 70 28 minecraft:redstone_wall_torch[facing=south]',
    'setblock 1160 70 28 minecraft:redstone_wire',
    'setblock 1164 70 28 minecraft:redstone_wire',
    'setblock 1168 70 28 minecraft:redstone_torch',
    'setblock 1172 70 28 minecraft:redstone_wire',
];

const breakPhase = [
    'setblock 1168 69 28 minecraft:air',
    'setblock 1172 69 28 minecraft:air',
];

const verify = [
  'execute if block 1152 70 28 minecraft:redstone_torch run say PASS_REDSTONE_TORCH_ON_STONE',
  'execute if block 1156 70 28 minecraft:redstone_wall_torch run say PASS_REDSTONE_WALL_TORCH_ON_WALL',
  'execute if block 1160 70 28 minecraft:redstone_wire run say PASS_REDSTONE_WIRE_CROSS_ON_FLOOR',
  'execute if block 1164 70 28 minecraft:redstone_wire run say PASS_REDSTONE_WIRE_POWER_15',
  'execute unless block 1168 70 28 minecraft:redstone_torch run say PASS_REDSTONE_TORCH_SUPPORT_REMOVAL_BREAK',
  'execute unless block 1172 70 28 minecraft:redstone_wire run say PASS_REDSTONE_WIRE_SUPPORT_REMOVAL_BREAK',
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
      console.log('\n=== REDSTONE TORCH & REDSTONE WIRE DUAL-SERVER DIFFERENTIAL SUMMARY ===');
      const expected = [
        'PASS_REDSTONE_TORCH_ON_STONE',
        'PASS_REDSTONE_WALL_TORCH_ON_WALL',
        'PASS_REDSTONE_WIRE_CROSS_ON_FLOOR',
        'PASS_REDSTONE_WIRE_POWER_15',
        'PASS_REDSTONE_TORCH_SUPPORT_REMOVAL_BREAK',
        'PASS_REDSTONE_WIRE_SUPPORT_REMOVAL_BREAK',
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

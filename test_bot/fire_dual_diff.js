const mc = require('minecraft-protocol');

function buildSetup() {
  return [
    'tp @s 965 85 28',
    'kill @e[type=item,x=950,y=60,z=20,dx=45,dy=35,dz=15]',
    'fill 950 67 25 995 76 31 air',
    'fill 950 68 25 995 68 31 minecraft:stone',
    
    // Foundations
    'setblock 952 69 28 minecraft:netherrack',
    'setblock 956 69 28 minecraft:soul_sand',
    'setblock 960 69 28 minecraft:soul_soil',
    'setblock 964 69 28 minecraft:stone',
    'setblock 968 69 28 minecraft:netherrack',
    'setblock 972 69 28 minecraft:stone',
  ];
}

const placementPhase = [
    'setblock 952 70 28 minecraft:fire',
    'setblock 956 70 28 minecraft:soul_fire',
    'setblock 960 70 28 minecraft:soul_fire',
    'setblock 964 70 28 minecraft:fire',
    'setblock 968 70 28 minecraft:fire[age=5]',
    'setblock 972 70 28 minecraft:fire',
];

const breakPhase = [
    'setblock 972 69 28 minecraft:air',
];

const verify = [
  'execute if block 952 70 28 minecraft:fire run say PASS_FIRE_ON_NETHERRACK',
  'execute if block 956 70 28 minecraft:soul_fire run say PASS_SOUL_FIRE_ON_SOUL_SAND',
  'execute if block 960 70 28 minecraft:soul_fire run say PASS_SOUL_FIRE_ON_SOUL_SOIL',
  'execute if block 964 70 28 minecraft:fire run say PASS_FIRE_ON_STONE',
  'execute if block 968 70 28 minecraft:fire run say PASS_FIRE_AGE_PROP',
  'execute unless block 972 70 28 minecraft:fire run say PASS_SUPPORT_REMOVAL_BREAK',
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
      console.log('\n=== FIRE & SOUL FIRE DUAL-SERVER DIFFERENTIAL SUMMARY ===');
      const expected = [
        'PASS_FIRE_ON_NETHERRACK',
        'PASS_SOUL_FIRE_ON_SOUL_SAND',
        'PASS_SOUL_FIRE_ON_SOUL_SOIL',
        'PASS_FIRE_ON_STONE',
        'PASS_FIRE_AGE_PROP',
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

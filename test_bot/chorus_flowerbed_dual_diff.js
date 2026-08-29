const mc = require('minecraft-protocol');

function buildSetup() {
  return [
    'tp @s 865 85 28',
    'kill @e[type=item,x=850,y=60,z=20,dx=45,dy=35,dz=15]',
    'fill 850 67 25 895 76 31 air',
    'fill 850 68 25 895 68 31 minecraft:stone',
    
    // Foundations
    'setblock 852 69 28 minecraft:end_stone',
    'setblock 856 69 28 minecraft:end_stone',
    'setblock 860 69 28 minecraft:grass_block',
    'setblock 864 69 28 minecraft:grass_block',
    'setblock 868 69 28 minecraft:dirt',
    'setblock 872 69 28 minecraft:end_stone',
  ];
}

const placementPhase = [
    'setblock 852 70 28 minecraft:chorus_plant',
    'setblock 852 71 28 minecraft:chorus_flower[age=0]',
    'setblock 856 70 28 minecraft:chorus_flower[age=0]',
    'setblock 860 70 28 minecraft:pink_petals[flower_amount=1]',
    'setblock 864 70 28 minecraft:pink_petals[flower_amount=4]',
    'setblock 868 70 28 minecraft:pink_petals[flower_amount=2]',
    'setblock 872 70 28 minecraft:chorus_flower[age=0]',
];

const breakPhase = [
    'setblock 872 69 28 minecraft:air',
];

const verify = [
  'execute if block 852 70 28 minecraft:chorus_plant run say PASS_CHORUS_PLANT_ON_END_STONE',
  'execute if block 852 71 28 minecraft:chorus_flower run say PASS_CHORUS_FLOWER_ON_CHORUS_PLANT',
  'execute if block 856 70 28 minecraft:chorus_flower run say PASS_CHORUS_FLOWER_ON_END_STONE',
  'execute if block 860 70 28 minecraft:pink_petals run say PASS_PINK_PETALS_ON_GRASS',
  'execute if block 864 70 28 minecraft:pink_petals run say PASS_PINK_PETALS_4_STACK',
  'execute if block 868 70 28 minecraft:pink_petals run say PASS_PINK_PETALS_2_STACK',
  'execute unless block 872 70 28 minecraft:chorus_flower run say PASS_SUPPORT_REMOVAL_BREAK',
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
      console.log('\n=== CHORUS, FLOWERBED & GROUND COVER DUAL-SERVER DIFFERENTIAL SUMMARY ===');
      const expected = [
        'PASS_CHORUS_PLANT_ON_END_STONE',
        'PASS_CHORUS_FLOWER_ON_CHORUS_PLANT',
        'PASS_CHORUS_FLOWER_ON_END_STONE',
        'PASS_PINK_PETALS_ON_GRASS',
        'PASS_PINK_PETALS_4_STACK',
        'PASS_PINK_PETALS_2_STACK',
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

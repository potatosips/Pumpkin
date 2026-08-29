const mc = require('minecraft-protocol');

function buildSetup() {
  return [
    'tp @s 815 85 28',
    'kill @e[type=item,x=800,y=60,z=20,dx=45,dy=35,dz=15]',
    'fill 800 67 25 845 76 31 air',
    'fill 800 68 25 845 68 31 minecraft:stone',
    
    // Foundations
    'setblock 802 69 28 minecraft:warped_nylium',
    
    'setblock 806 72 28 minecraft:netherrack',
    
    'setblock 810 69 28 minecraft:sand',
    'setblock 810 70 28 minecraft:water',
    'setblock 810 71 28 minecraft:water',
    
    'setblock 814 69 28 minecraft:dirt',
    'setblock 814 70 28 minecraft:water',
    
    'setblock 818 69 28 minecraft:dirt',
    'setblock 818 70 28 minecraft:water',
    'setblock 818 71 28 minecraft:water',
    
    'setblock 822 69 28 minecraft:warped_nylium',
  ];
}

const placementPhase = [
    'setblock 802 70 28 minecraft:twisting_vines',
    'setblock 802 71 28 minecraft:twisting_vines',
    'setblock 806 71 28 minecraft:weeping_vines',
    'setblock 806 70 28 minecraft:weeping_vines',
    'setblock 810 70 28 minecraft:kelp',
    'setblock 814 70 28 minecraft:seagrass',
    'setblock 818 70 28 minecraft:tall_seagrass[half=lower]',
    'setblock 818 71 28 minecraft:tall_seagrass[half=upper]',
    'setblock 822 70 28 minecraft:twisting_vines',
];

const breakPhase = [
    'setblock 822 69 28 minecraft:air',
];

const verify = [
  'execute if block 802 70 28 minecraft:twisting_vines_plant run say PASS_TWISTING_VINES_PLANT_BASE',
  'execute if block 802 71 28 minecraft:twisting_vines run say PASS_TWISTING_VINES_TOP',
  'execute if block 806 71 28 minecraft:weeping_vines_plant run say PASS_WEEPING_VINES_PLANT_TOP',
  'execute if block 806 70 28 minecraft:weeping_vines run say PASS_WEEPING_VINES_BOTTOM',
  'execute if block 810 70 28 minecraft:kelp run say PASS_KELP_SUBMERGED_ON_SAND',
  'execute if block 814 70 28 minecraft:seagrass run say PASS_SEAGRASS_SUBMERGED_ON_DIRT',
  'execute if block 818 70 28 minecraft:tall_seagrass run say PASS_TALL_SEAGRASS_LOWER',
  'execute if block 818 71 28 minecraft:tall_seagrass run say PASS_TALL_SEAGRASS_UPPER',
  'execute unless block 822 70 28 minecraft:twisting_vines run say PASS_SUPPORT_REMOVAL_BREAK',
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
      console.log('\n=== VINES, KELP & SEAGRASS DUAL-SERVER DIFFERENTIAL SUMMARY ===');
      const expected = [
        'PASS_TWISTING_VINES_PLANT_BASE',
        'PASS_TWISTING_VINES_TOP',
        'PASS_WEEPING_VINES_PLANT_TOP',
        'PASS_WEEPING_VINES_BOTTOM',
        'PASS_KELP_SUBMERGED_ON_SAND',
        'PASS_SEAGRASS_SUBMERGED_ON_DIRT',
        'PASS_TALL_SEAGRASS_LOWER',
        'PASS_TALL_SEAGRASS_UPPER',
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

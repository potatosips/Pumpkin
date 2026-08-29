const mc = require('minecraft-protocol');

function buildSetup() {
  return [
    'tp @s 715 85 28',
    'kill @e[type=item,x=700,y=60,z=20,dx=45,dy=35,dz=15]',
    'fill 700 67 25 745 76 31 air',
    'fill 700 68 25 745 68 31 minecraft:stone',
    
    // Foundations
    'setblock 702 69 28 minecraft:tube_coral_block',
    'setblock 702 70 28 minecraft:water',
    
    'setblock 706 69 28 minecraft:dirt',
    
    'setblock 710 69 28 minecraft:stone',
    
    'setblock 714 70 28 minecraft:water',
    
    'setblock 718 71 28 minecraft:stone',
    
    'setblock 722 71 28 minecraft:dirt',
    
    'setblock 726 71 28 minecraft:stone',
  ];
}

const placementPhase = [
    'setblock 702 70 28 minecraft:sea_pickle[waterlogged=true,pickles=1]',
    'setblock 706 70 28 minecraft:sea_pickle[waterlogged=false,pickles=1]',
    'setblock 710 70 28 minecraft:sea_pickle[waterlogged=false,pickles=4]',
    'setblock 714 71 28 minecraft:lily_pad',
    'setblock 718 70 28 minecraft:spore_blossom',
    'setblock 722 70 28 minecraft:spore_blossom',
    'setblock 726 70 28 minecraft:spore_blossom',
];

const breakPhase = [
    'setblock 726 71 28 minecraft:air',
];

const verify = [
  'execute if block 702 70 28 minecraft:sea_pickle run say PASS_SEA_PICKLE_ON_CORAL_SUBMERGED',
  'execute if block 706 70 28 minecraft:sea_pickle run say PASS_SEA_PICKLE_ON_DIRT_DRY',
  'execute if block 710 70 28 minecraft:sea_pickle run say PASS_SEA_PICKLE_4_STACK',
  'execute if block 714 71 28 minecraft:lily_pad run say PASS_LILY_PAD_ON_WATER',
  'execute if block 718 70 28 minecraft:spore_blossom run say PASS_SPORE_BLOSSOM_CEILING_STONE',
  'execute if block 722 70 28 minecraft:spore_blossom run say PASS_SPORE_BLOSSOM_CEILING_DIRT',
  'execute unless block 726 70 28 minecraft:spore_blossom run say PASS_SUPPORT_REMOVAL_BREAK',
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
      console.log('\n=== AQUATIC & HANGING DUAL-SERVER DIFFERENTIAL SUMMARY ===');
      const expected = [
        'PASS_SEA_PICKLE_ON_CORAL_SUBMERGED',
        'PASS_SEA_PICKLE_ON_DIRT_DRY',
        'PASS_SEA_PICKLE_4_STACK',
        'PASS_LILY_PAD_ON_WATER',
        'PASS_SPORE_BLOSSOM_CEILING_STONE',
        'PASS_SPORE_BLOSSOM_CEILING_DIRT',
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

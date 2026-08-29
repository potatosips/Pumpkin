const mc = require('minecraft-protocol');

function buildSetup() {
  return [
    'tp @s 180 85 28',
    'kill @e[type=item,x=175,y=60,z=26,dx=20,dy=50,dz=5]',
    'fill 175 60 25 190 85 32 air',
    
    // Solid base blocks:
    'setblock 176 69 28 minecraft:stone',
    'setblock 177 69 28 minecraft:stone',
    'setblock 178 71 28 minecraft:stone',
    'setblock 179 70 29 minecraft:stone',
    'setblock 180 69 28 minecraft:stone',
    'setblock 181 69 28 minecraft:stone',
    'setblock 182 69 28 minecraft:stone',
    'setblock 185 69 28 minecraft:stone',
    
    // Clusters and Buds:
    // Floor cluster (facing up on stone at 69):
    'setblock 176 70 28 minecraft:amethyst_cluster[facing=up]',
    
    // Waterlogged floor cluster:
    'setblock 177 70 28 minecraft:amethyst_cluster[facing=up,waterlogged=true]',
    
    // Ceiling cluster (facing down from stone at 71):
    'setblock 178 70 28 minecraft:amethyst_cluster[facing=down]',
    
    // Wall cluster (facing north from stone at Z=29):
    'setblock 179 70 28 minecraft:amethyst_cluster[facing=north]',
    
    // Bud stages:
    'setblock 180 70 28 minecraft:small_amethyst_bud[facing=up]',
    'setblock 181 70 28 minecraft:medium_amethyst_bud[facing=up]',
    'setblock 182 70 28 minecraft:large_amethyst_bud[facing=up]',
    
    // Place cluster on support then break support:
    'setblock 185 70 28 minecraft:amethyst_cluster[facing=up]',
    'setblock 185 69 28 minecraft:air',
  ];
}

const setup = buildSetup();
const verify = [
  'execute if block 176 70 28 minecraft:amethyst_cluster[facing=up] run say PASS_FLOOR_CLUSTER',
  'execute if block 177 70 28 minecraft:amethyst_cluster[facing=up,waterlogged=true] run say PASS_WATERLOGGED_CLUSTER',
  'execute if block 178 70 28 minecraft:amethyst_cluster[facing=down] run say PASS_CEILING_CLUSTER',
  'execute if block 179 70 28 minecraft:amethyst_cluster[facing=north] run say PASS_WALL_CLUSTER',
  'execute if block 180 70 28 minecraft:small_amethyst_bud[facing=up] run say PASS_SMALL_BUD',
  'execute if block 181 70 28 minecraft:medium_amethyst_bud[facing=up] run say PASS_MEDIUM_BUD',
  'execute if block 182 70 28 minecraft:large_amethyst_bud[facing=up] run say PASS_LARGE_BUD',
  // At 185 70 28 support stone was removed -> cluster should break
  'execute unless block 185 70 28 minecraft:amethyst_cluster run say PASS_UNSUPPORTED_REMOVED',
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
      setup.forEach((command, index) => setTimeout(() => {
        client.write('chat_command', { command, timestamp: BigInt(Date.now()) });
      }, index * 100));

      const verifyStart = setup.length * 100 + 2500;
      verify.forEach((command, index) => setTimeout(() => {
        client.write('chat_command', { command, timestamp: BigInt(Date.now()) });
      }, verifyStart + index * 150));

      setTimeout(() => client.end(), verifyStart + verify.length * 150 + 1000);
    }, 500);
  });

  client.on('system_chat', packet => handleMsg(name, packet.content));
  client.on('profileless_chat', packet => handleMsg(name, packet.message));
  client.on('disguised_chat', packet => handleMsg(name, packet.message));
  client.on('player_chat', packet => handleMsg(name, packet.unsignedContent || packet.plainMessage || packet.signedChatContent || packet));

  client.on('error', error => console.error(`[${name}] ERROR ${error.message}`));
  client.on('end', () => {
    if (++finished === 2) {
      console.log('\n=== AMETHYST DUAL-SERVER DIFFERENTIAL SUMMARY ===');
      const expected = [
        'PASS_FLOOR_CLUSTER',
        'PASS_WATERLOGGED_CLUSTER',
        'PASS_CEILING_CLUSTER',
        'PASS_WALL_CLUSTER',
        'PASS_SMALL_BUD',
        'PASS_MEDIUM_BUD',
        'PASS_LARGE_BUD',
        'PASS_UNSUPPORTED_REMOVED',
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

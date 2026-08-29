const mc = require('minecraft-protocol');

function buildSetup() {
  return [
    'tp @s 190 85 28',
    'kill @e[type=item,x=185,y=60,z=25,dx=20,dy=50,dz=10]',
    'fill 185 60 25 205 85 35 air',
    
    // Soul Sand Upward Column Setup (3 blocks high):
    'fill 187 68 27 189 72 29 minecraft:glass',
    'setblock 188 68 28 minecraft:soul_sand',
    'setblock 188 69 28 minecraft:water',
    'setblock 188 70 28 minecraft:water',
    'setblock 188 71 28 minecraft:water',
    
    // Magma Block Downward Column Setup (3 blocks high):
    'fill 191 68 27 193 72 29 minecraft:glass',
    'setblock 192 68 28 minecraft:magma_block',
    'setblock 192 69 28 minecraft:water',
    'setblock 192 70 28 minecraft:water',
    'setblock 192 71 28 minecraft:water',
    
    // Removal / Reversion Setup:
    'fill 195 68 27 197 71 29 minecraft:glass',
    'setblock 196 68 28 minecraft:soul_sand',
    'setblock 196 69 28 minecraft:water',
    'setblock 196 70 28 minecraft:water',
    // Now replace soul sand with stone to trigger column destruction & reversion to water
    'setblock 196 68 28 minecraft:stone',
  ];
}

const setup = buildSetup();
const verify = [
  // Soul sand bubble column propagation across 3 levels:
  'execute if block 188 69 28 minecraft:bubble_column[drag=false] run say PASS_SOUL_SAND_LOWER',
  'execute if block 188 70 28 minecraft:bubble_column[drag=false] run say PASS_SOUL_SAND_MID',
  'execute if block 188 71 28 minecraft:bubble_column[drag=false] run say PASS_SOUL_SAND_TOP',
  
  // Magma block whirlpool column propagation across 3 levels:
  'execute if block 192 69 28 minecraft:bubble_column[drag=true] run say PASS_MAGMA_LOWER',
  'execute if block 192 70 28 minecraft:bubble_column[drag=true] run say PASS_MAGMA_MID',
  'execute if block 192 71 28 minecraft:bubble_column[drag=true] run say PASS_MAGMA_TOP',
  
  // Removed soul sand should revert bubble column back to water:
  'execute if block 196 69 28 minecraft:water run say PASS_REVERT_LOWER_WATER',
  'execute if block 196 70 28 minecraft:water run say PASS_REVERT_UPPER_WATER',
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

      const verifyStart = setup.length * 100 + 3500;
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
      console.log('\n=== BUBBLE COLUMN DUAL-SERVER DIFFERENTIAL SUMMARY ===');
      const expected = [
        'PASS_SOUL_SAND_LOWER',
        'PASS_SOUL_SAND_MID',
        'PASS_SOUL_SAND_TOP',
        'PASS_MAGMA_LOWER',
        'PASS_MAGMA_MID',
        'PASS_MAGMA_TOP',
        'PASS_REVERT_LOWER_WATER',
        'PASS_REVERT_UPPER_WATER',
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

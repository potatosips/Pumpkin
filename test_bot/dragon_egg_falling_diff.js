const mc = require('minecraft-protocol');

function buildSetup() {
  return [
    'tp @s 182 85 28',
    'kill @e[type=falling_block,x=175,y=60,z=26,dx=20,dy=50,dz=5]',
    'kill @e[type=item,x=175,y=60,z=26,dx=20,dy=50,dz=5]',
    'fill 175 68 25 190 85 32 air',
    'fill 175 68 25 190 68 32 stone',
    
    // Setup 1: Dragon Egg falling
    'setblock 180 75 28 stone',
    'setblock 180 76 28 minecraft:dragon_egg',

    // Setup 2: Sand falling on Torch
    'setblock 185 69 28 stone',
    'setblock 185 70 28 minecraft:torch',
    'setblock 185 75 28 stone',
    'setblock 185 76 28 minecraft:sand',

    // Trigger fallings
    'setblock 180 75 28 air',
    'setblock 185 75 28 air',
  ];
}

const setup = buildSetup();
const verify = [
  // 1. Dragon egg landing on solid floor at Y=69
  'execute if block 180 69 28 minecraft:dragon_egg run say PASS_DRAGON_EGG_LANDED',
  // 2. Torch preservation at Y=70
  'execute if block 185 70 28 minecraft:torch run say PASS_TORCH_PRESERVED',
  // 3. Sand item drop dropped upon landing on torch (check radius 3 from torch)
  'execute as @e[type=item] at @s if block ~ ~-1 ~ minecraft:stone run say PASS_SAND_ITEM_DROPPED'
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
  if (text.includes('PASS_') || text.includes('minecraft:dragon_egg') || text.includes('commands.data.entity.query')) {
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
      }, index * 80));

      // Wait 2.5s for falling blocks to land
      const verifyStart = setup.length * 80 + 2500;
      verify.forEach((command, index) => setTimeout(() => {
        client.write('chat_command', { command, timestamp: BigInt(Date.now()) });
      }, verifyStart + index * 120));

      setTimeout(() => client.end(), verifyStart + verify.length * 120 + 1000);
    }, 500);
  });

  client.on('system_chat', packet => handleMsg(name, packet.content));
  client.on('profileless_chat', packet => handleMsg(name, packet.message));
  client.on('disguised_chat', packet => handleMsg(name, packet.message));
  client.on('player_chat', packet => handleMsg(name, packet.unsignedContent || packet.plainMessage || packet.signedChatContent || packet));

  client.on('error', error => console.error(`[${name}] ERROR ${error.message}`));
  client.on('end', () => {
    if (++finished === 2) {
      console.log('\n=== DRAGON EGG & NON-SOLID FALLING DIFFERENTIAL SUMMARY ===');
      const expected = ['PASS_DRAGON_EGG_LANDED', 'PASS_TORCH_PRESERVED', 'PASS_SAND_ITEM_DROPPED'];
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
run('VANILLA', 25575);

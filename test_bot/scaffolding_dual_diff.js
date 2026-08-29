const mc = require('minecraft-protocol');

function buildSetup() {
  return [
    'tp @s 182 85 28',
    'kill @e[type=falling_block,x=175,y=60,z=26,dx=20,dy=50,dz=5]',
    'kill @e[type=item,x=175,y=60,z=26,dx=20,dy=50,dz=5]',
    'fill 175 68 25 190 85 32 air',
    'fill 175 68 25 190 68 32 stone',
    
    // Pillar on stone:
    'setblock 180 69 28 minecraft:scaffolding',
    'setblock 180 70 28 minecraft:scaffolding',
    
    // Horizontal branches:
    'setblock 181 70 28 minecraft:scaffolding',
    'setblock 182 70 28 minecraft:scaffolding',
    'setblock 183 70 28 minecraft:scaffolding',
    'setblock 184 70 28 minecraft:scaffolding',
    'setblock 185 70 28 minecraft:scaffolding',
    'setblock 186 70 28 minecraft:scaffolding',
    'setblock 187 70 28 minecraft:scaffolding',
  ];
}

const setup = buildSetup();
const verify = [
  'execute if block 180 69 28 minecraft:scaffolding run say PASS_COL_BASE',
  'execute if block 180 70 28 minecraft:scaffolding run say PASS_COL_TOP',
  'execute if block 181 70 28 minecraft:scaffolding run say PASS_BRANCH_1',
  'execute if block 182 70 28 minecraft:scaffolding run say PASS_BRANCH_2',
  'execute if block 183 70 28 minecraft:scaffolding run say PASS_BRANCH_3',
  'execute if block 184 70 28 minecraft:scaffolding run say PASS_BRANCH_4',
  'execute if block 185 70 28 minecraft:scaffolding run say PASS_BRANCH_5',
  'execute if block 186 70 28 minecraft:scaffolding run say PASS_BRANCH_6',
  'execute unless block 187 70 28 minecraft:scaffolding run say PASS_UNSTABLE_FALL',
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
      }, index * 80));

      const verifyStart = setup.length * 80 + 2000;
      verify.forEach((command, index) => setTimeout(() => {
        client.write('chat_command', { command, timestamp: BigInt(Date.now()) });
      }, verifyStart + index * 100));

      setTimeout(() => client.end(), verifyStart + verify.length * 100 + 1000);
    }, 500);
  });

  client.on('system_chat', packet => handleMsg(name, packet.content));
  client.on('profileless_chat', packet => handleMsg(name, packet.message));
  client.on('disguised_chat', packet => handleMsg(name, packet.message));
  client.on('player_chat', packet => handleMsg(name, packet.unsignedContent || packet.plainMessage || packet.signedChatContent || packet));

  client.on('error', error => console.error(`[${name}] ERROR ${error.message}`));
  client.on('end', () => {
    if (++finished === 2) {
      console.log('\n=== SCAFFOLDING MECHANICS DUAL-SERVER DIFFERENTIAL SUMMARY ===');
      const expected = [
        'PASS_COL_BASE',
        'PASS_COL_TOP',
        'PASS_BRANCH_1',
        'PASS_BRANCH_2',
        'PASS_BRANCH_3',
        'PASS_BRANCH_4',
        'PASS_BRANCH_5',
        'PASS_BRANCH_6',
        'PASS_UNSTABLE_FALL',
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

const mc = require('minecraft-protocol');

function buildSetup() {
  return [
    'tp @s 180 85 28',
    'kill @e[type=falling_block,x=175,y=60,z=26,dx=20,dy=50,dz=5]',
    'kill @e[type=item,x=175,y=60,z=26,dx=20,dy=50,dz=5]',
    'fill 175 60 25 190 85 32 air',
    'fill 175 68 25 190 68 32 stone',
    'fill 175 80 25 190 80 32 stone',
    
    // 1-block stalactite at X=176:
    'setblock 176 79 28 minecraft:pointed_dripstone[vertical_direction=down]',
    
    // 2-block stalactite at X=177:
    'setblock 177 79 28 minecraft:pointed_dripstone[vertical_direction=down]',
    'setblock 177 78 28 minecraft:pointed_dripstone[vertical_direction=down]',

    // 3-block stalactite at X=178:
    'setblock 178 79 28 minecraft:pointed_dripstone[vertical_direction=down]',
    'setblock 178 78 28 minecraft:pointed_dripstone[vertical_direction=down]',
    'setblock 178 77 28 minecraft:pointed_dripstone[vertical_direction=down]',

    // Stalagmite on floor at X=180:
    'setblock 180 69 28 minecraft:pointed_dripstone[vertical_direction=up]',
    
    // Falling stalactite trigger at X=185:
    'setblock 185 75 28 minecraft:pointed_dripstone[vertical_direction=down]',
  ];
}

const setup = buildSetup();
const verify = [
  'execute if block 176 79 28 minecraft:pointed_dripstone run say PASS_1_TIP',
  'execute if block 177 79 28 minecraft:pointed_dripstone run say PASS_2_TOP',
  'execute if block 177 78 28 minecraft:pointed_dripstone run say PASS_2_BOT',
  'execute if block 178 79 28 minecraft:pointed_dripstone run say PASS_3_TOP',
  'execute if block 178 78 28 minecraft:pointed_dripstone run say PASS_3_MID',
  'execute if block 178 77 28 minecraft:pointed_dripstone run say PASS_3_BOT',
  'execute if block 180 69 28 minecraft:pointed_dripstone run say PASS_STALAGMITE_TIP',
  'execute unless block 185 75 28 minecraft:pointed_dripstone run say PASS_FALLING_DRIPSTONE',
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
      console.log('\n=== POINTED DRIPSTONE DUAL-SERVER DIFFERENTIAL SUMMARY ===');
      const expected = [
        'PASS_1_TIP',
        'PASS_2_TOP',
        'PASS_2_BOT',
        'PASS_3_TOP',
        'PASS_3_MID',
        'PASS_3_BOT',
        'PASS_STALAGMITE_TIP',
        'PASS_FALLING_DRIPSTONE',
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

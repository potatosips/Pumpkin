const mc = require('minecraft-protocol');

const cases = [
  // Test 1: Horizontal water neighbor -> converts to solid
  { name: 'white_concrete_powder_water_neighbor', x: 170, block: 'minecraft:white_concrete_powder', waterOffset: { x: -1, y: 0, z: 0 }, expectSolid: 'minecraft:white_concrete' },
  // Test 2: Water directly above -> converts to solid
  { name: 'cyan_concrete_powder_water_above', x: 175, block: 'minecraft:cyan_concrete_powder', waterOffset: { x: 0, y: 1, z: 0 }, expectSolid: 'minecraft:cyan_concrete' },
  // Test 3: Water diagonal-below (on stone floor at y=74) -> stays powder
  { name: 'yellow_concrete_powder_water_below_diag', x: 180, block: 'minecraft:yellow_concrete_powder', waterOffset: { x: 1, y: -1, z: 0 }, expectPowder: 'minecraft:yellow_concrete_powder' },
];

function buildSetup() {
  const cmds = [
    'tp @s 177 95 28',
    'kill @e[type=falling_block,x=165,y=60,z=26,dx=30,dy=160,dz=5]',
    'fill 165 70 25 195 115 32 air',
    'fill 165 69 25 195 74 32 stone', // Solid floor up to 74
  ];

  for (const c of cases) {
    cmds.push(`setblock ${c.x + c.waterOffset.x} ${75 + c.waterOffset.y} ${28 + c.waterOffset.z} water`);
    cmds.push(`setblock ${c.x} 75 28 ${c.block}`);
  }

  return cmds;
}

const setup = buildSetup();
const verify = [
  // Block state verifications
  { name: 'white_neighbor_solid', command: 'execute if block 170 75 28 minecraft:white_concrete run say PASS_WHITE_SOLID' },
  { name: 'cyan_above_solid', command: 'execute if block 175 75 28 minecraft:cyan_concrete run say PASS_CYAN_SOLID' },
  { name: 'yellow_below_powder', command: 'execute if block 180 75 28 minecraft:yellow_concrete_powder run say PASS_YELLOW_POWDER' },
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
  if (text.includes('PASS_WHITE_SOLID') || text.includes('PASS_CYAN_SOLID') || text.includes('PASS_YELLOW_POWDER')) {
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

      const verifyStart = setup.length * 100 + 1000;
      verify.forEach(({ name: caseName, command }, index) => setTimeout(() => {
        client.write('chat_command', { command, timestamp: BigInt(Date.now()) });
      }, verifyStart + index * 150));

      setTimeout(() => client.end(), verifyStart + verify.length * 150 + 1000);
    }, 500);
  });

  client.on('system_chat', packet => handleMsg(name, packet.content));
  client.on('profileless_chat', packet => handleMsg(name, packet.message));

  client.on('error', error => console.error(`[${name}] ERROR ${error.message}`));
  client.on('end', () => {
    if (++finished === 2) {
      console.log('\n=== DIFFERENTIAL RESULTS SUMMARY ===');
      const expected = ['PASS_WHITE_SOLID', 'PASS_CYAN_SOLID', 'PASS_YELLOW_POWDER'];
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

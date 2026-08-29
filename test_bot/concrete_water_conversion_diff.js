const mc = require('minecraft-protocol');

function summarize(node) {
  if (node == null) return '';
  if (typeof node !== 'object') return String(node);
  if (node.type && node.type !== 'compound' && node.type !== 'list') return summarize(node.value);
  if (node.type === 'list') return (node.value?.value ?? node.value ?? []).map(summarize).join('');
  if (node.type === 'compound') return Object.values(node.value ?? {}).map(summarize).filter(Boolean).join('|');
  return Object.values(node).map(summarize).filter(Boolean).join('|');
}

function runServer(port, label) {
  return new Promise((resolve, reject) => {
    const client = mc.createClient({ host: '127.0.0.1', port, username: 'TestBot', version: '1.21.4', auth: 'offline' });
    const log = [];

    const testCmds = [
      // Clean test areas
      'fill 5 69 5 45 85 20 air',
      'fill 5 68 5 45 68 20 stone',
      
      // Test 1: Horizontal water neighbor conversion (at X=10)
      'setblock 9 69 10 water',
      'setblock 10 69 10 white_concrete_powder',
      
      // Test 2: Water directly above conversion (at X=20)
      'setblock 20 70 10 water',
      'setblock 20 69 10 orange_concrete_powder',

      // Test 3: Water below only (should NOT solidify immediately - at X=30)
      'setblock 30 68 10 water',
      'setblock 30 69 10 yellow_concrete_powder',

      // Test 4: Falling concrete powder falling into water (at X=40)
      'setblock 40 69 10 water',
      'setblock 40 76 10 magenta_concrete_powder'
    ];

    const verifyCmds = [
      'execute if block 10 69 10 minecraft:white_concrete run tellraw @a "MATCH_WHITE_CONCRETE"',
      'execute if block 20 69 10 minecraft:orange_concrete run tellraw @a "MATCH_ORANGE_CONCRETE"',
      'execute if block 30 69 10 minecraft:yellow_concrete_powder run tellraw @a "MATCH_YELLOW_POWDER"',
      'execute if block 40 69 10 minecraft:magenta_concrete run tellraw @a "MATCH_MAGENTA_CONCRETE"'
    ];

    client.on('system_chat', packet => {
      const text = summarize(packet.content);
      if (text.includes('MATCH_')) {
        log.push(text);
      }
    });

    client.on('position', async () => {
      try {
        await sleep(500);
        await sendCmd(client, 'tp @s 25 75 10');

        for (const cmd of testCmds) {
          await sendCmd(client, cmd);
          await sleep(100);
        }

        // Wait 1.5s for falling entity to fall and solidify
        await sleep(1500);

        for (const v of verifyCmds) {
          await sendCmd(client, v);
          await sleep(120);
        }

        setTimeout(() => {
          client.end();
          resolve(log);
        }, 500);
      } catch (err) {
        reject(err);
      }
    });

    client.on('error', reject);
  });
}

function sleep(ms) {
  return new Promise(res => setTimeout(res, ms));
}

function sendCmd(client, command) {
  return new Promise(res => {
    client.write('chat_command', { command, timestamp: BigInt(Date.now()) });
    setTimeout(res, 60);
  });
}

async function main() {
  console.log('--- Step 1: Testing Concrete Water Conversion on Pumpkin (25565) ---');
  const pLog = await runServer(25565, 'PUMPKIN');
  console.log('--- Step 2: Testing Concrete Water Conversion on Vanilla (25575) ---');
  const vLog = await runServer(25575, 'VANILLA');

  console.log('\n--- Step 3: Comparison Matrix ---');
  const expected = [
    'MATCH_WHITE_CONCRETE',
    'MATCH_ORANGE_CONCRETE',
    'MATCH_YELLOW_POWDER',
    'MATCH_MAGENTA_CONCRETE'
  ];

  let matchCount = 0;
  for (const exp of expected) {
    const pHas = pLog.some(l => l.includes(exp));
    const vHas = vLog.some(l => l.includes(exp));
    const matched = pHas && vHas;
    if (matched) matchCount++;
    console.log(`[TEST: ${exp}]`);
    console.log(`  Pumpkin: ${pHas ? 'PASSED (MATCH)' : 'FAILED'}`);
    console.log(`  Vanilla: ${vHas ? 'PASSED (MATCH)' : 'FAILED'}`);
    console.log(`  Status:  ${matched ? '100% PARITY' : 'MISMATCH'}\n`);
  }
  console.log(`Total Parity Score: ${matchCount}/${expected.length} (${matchCount === expected.length ? '100% PARITY' : 'MISMATCH'})`);
}

main().catch(err => {
  console.error('Fatal error:', err);
  process.exit(1);
});

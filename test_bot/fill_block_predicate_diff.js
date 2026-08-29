const mc = require('minecraft-protocol');

function summarize(node) {
  if (node == null) return '';
  if (typeof node !== 'object') return String(node);
  if (node.type && node.type !== 'compound' && node.type !== 'list') return summarize(node.value);
  if (node.type === 'list') return (node.value?.value ?? node.value ?? []).map(summarize).join('');
  return Object.values(node.value ?? node).map(summarize).filter(Boolean).join('|');
}

function runBot(port, label) {
  return new Promise((resolve, reject) => {
    const client = mc.createClient({ host: '127.0.0.1', port, username: 'TestBot', version: '1.21.4', auth: 'offline' });
    const log = [];
    const commands = [
      'setblock ~ ~-1 ~ minecraft:stone',
      'setblock ~ ~ ~ minecraft:anvil[facing=east]',
      'fill ~1 ~ ~ ~3 ~ ~2 minecraft:oak_stairs[facing=south]',
      'fill ~1 ~ ~ ~3 ~ ~2 minecraft:stone replace minecraft:oak_stairs[facing=south]',
      'fill ~1 ~ ~ ~3 ~ ~2 minecraft:diamond_block replace minecraft:oak_stairs[facing=north]',
      'fill ~1 ~ ~ ~3 ~ ~2 minecraft:oak_log[axis=y]',
      'fill ~1 ~ ~ ~3 ~ ~2 minecraft:gold_block replace #minecraft:logs[axis=y]'
    ];
    let cmdIdx = 0;

    client.on('position', () => {
      setTimeout(sendNext, 300);
    });

    function sendNext() {
      if (cmdIdx >= commands.length) {
        setTimeout(() => {
          client.end();
          resolve(log);
        }, 600);
        return;
      }
      const cmd = commands[cmdIdx];
      client.write('chat_command', { command: cmd, timestamp: BigInt(Date.now()) });
    }

    client.on('system_chat', packet => {
      const text = summarize(packet.content);
      if (text.includes('multiplayer.player.joined')) return;
      if (cmdIdx < commands.length) {
        const cmd = commands[cmdIdx];
        log.push({ cmd, response: text });
        cmdIdx++;
        setTimeout(sendNext, 80);
      }
    });

    client.on('error', err => {
      console.error(`[${label}] Error:`, err.message);
      reject(err);
    });
  });
}

async function main() {
  console.log('--- Step 1: Running fill & block predicate tests on Pumpkin (25565) ---');
  const pumpkinLog = await runBot(25565, 'PUMPKIN');
  console.log('--- Step 2: Running fill & block predicate tests on Vanilla (25575) ---');
  const vanillaLog = await runBot(25575, 'VANILLA');

  console.log('\n--- Step 3: Comparison Matrix ---');
  let matchCount = 0;
  for (let i = 0; i < pumpkinLog.length; i++) {
    const p = pumpkinLog[i];
    const v = vanillaLog[i] || { cmd: p.cmd, response: '<missing>' };
    const pKey = p.response.includes('success') ? 'SUCCESS' : p.response.includes('failed') ? 'FAILED' : p.response;
    const vKey = v.response.includes('success') ? 'SUCCESS' : v.response.includes('failed') ? 'FAILED' : v.response;
    const matched = pKey === vKey;
    if (matched) matchCount++;
    console.log(`> CMD: ${p.cmd}`);
    console.log(`  [PUMPKIN] ${p.response}`);
    console.log(`  [VANILLA] ${v.response}`);
    console.log(`  STATUS: ${matched ? 'EXACT MATCH' : 'MISMATCH'}\n`);
  }
  console.log(`Parity Score: ${matchCount}/${pumpkinLog.length} (${matchCount === pumpkinLog.length ? '100% PARITY' : 'MISMATCH'})`);
}

main().catch(err => {
  console.error('Fatal error:', err);
  process.exit(1);
});

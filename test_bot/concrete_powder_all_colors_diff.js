const mc = require('minecraft-protocol');

const colors = [
  'white', 'orange', 'magenta', 'light_blue',
  'yellow', 'lime', 'pink', 'gray',
  'light_gray', 'cyan', 'purple', 'blue',
  'brown', 'green', 'red', 'black'
];

function buildSetup() {
  const cmds = [
    'tp @s 180 95 28',
    'fill 160 70 25 210 115 32 air',
    'fill 160 69 25 210 74 32 stone',
  ];

  colors.forEach((color, idx) => {
    const x = 162 + idx * 3;
    // Water adjacent at x-1
    cmds.push(`setblock ${x - 1} 75 28 water`);
    cmds.push(`setblock ${x} 75 28 minecraft:${color}_concrete_powder`);
  });

  return cmds;
}

const setup = buildSetup();
const verify = colors.map((color, idx) => {
  const x = 162 + idx * 3;
  return {
    color,
    command: `execute if block ${x} 75 28 minecraft:${color}_concrete run say PASS_${color.toUpperCase()}_CONCRETE`
  };
});

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
  if (text.includes('PASS_') && text.includes('_CONCRETE')) {
    results[name].push(text);
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

      const verifyStart = setup.length * 80 + 1000;
      verify.forEach(({ color, command }, index) => setTimeout(() => {
        client.write('chat_command', { command, timestamp: BigInt(Date.now()) });
      }, verifyStart + index * 100));

      setTimeout(() => client.end(), verifyStart + verify.length * 100 + 1000);
    }, 500);
  });

  client.on('system_chat', packet => handleMsg(name, packet.content));
  client.on('profileless_chat', packet => handleMsg(name, packet.message));

  client.on('error', error => console.error(`[${name}] ERROR ${error.message}`));
  client.on('end', () => {
    if (++finished === 2) {
      console.log('\n=== 16-COLOR CONCRETE POWDER WATER CONVERSION MATRIX ===');
      let matchCount = 0;
      colors.forEach(color => {
        const token = `PASS_${color.toUpperCase()}_CONCRETE`;
        const pHas = results.PUMPKIN.some(l => l.includes(token));
        const vHas = results.VANILLA.some(l => l.includes(token));
        const matched = pHas && vHas;
        if (matched) matchCount++;
        console.log(`[COLOR: ${color}] -> Pumpkin: ${pHas ? 'PASSED' : 'FAILED'} | Vanilla: ${vHas ? 'PASSED' : 'FAILED'} | ${matched ? '100% PARITY' : 'MISMATCH'}`);
      });
      console.log(`\nOverall Parity Score: ${matchCount}/${colors.length} (${matchCount === colors.length ? '100% PARITY' : 'MISMATCH'})`);
      process.exit(matchCount === colors.length ? 0 : 1);
    }
  });
}

run('PUMPKIN', 25565);
run('VANILLA', 25575);

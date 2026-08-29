const mc = require('minecraft-protocol');

const cases = [
  { name: 'sand', block: 'minecraft:sand', x: 170 },
  { name: 'red_sand', block: 'minecraft:red_sand', x: 173 },
  { name: 'gravel', block: 'minecraft:gravel', x: 176 },
  { name: 'white_concrete_powder', block: 'minecraft:white_concrete_powder', x: 179 },
  { name: 'black_concrete_powder', block: 'minecraft:black_concrete_powder', x: 182 },
  { name: 'cyan_concrete_powder', block: 'minecraft:cyan_concrete_powder', x: 185 },
  { name: 'lime_concrete_powder', block: 'minecraft:lime_concrete_powder', x: 188 },
  { name: 'anvil_north', block: 'minecraft:anvil[facing=north]', x: 191 },
  { name: 'anvil_south', block: 'minecraft:anvil[facing=south]', x: 194 },
  { name: 'chipped_anvil_east', block: 'minecraft:chipped_anvil[facing=east]', x: 197 },
  { name: 'damaged_anvil_west', block: 'minecraft:damaged_anvil[facing=west]', x: 200 },
  { name: 'pointed_dripstone', block: 'minecraft:pointed_dripstone[vertical_direction=down,thickness=tip,waterlogged=false]', x: 203, isDripstone: true }
];

const selector = x => `@e[type=falling_block,x=${x},y=70,z=28,dx=1,dy=160,dz=1,limit=1]`;

function buildSetup() {
  const cmds = [
    'tp @s 188 90 28',
    'kill @e[type=falling_block,x=165,y=60,z=26,dx=45,dy=160,dz=5]',
    'fill 165 70 26 210 108 30 air',
    'fill 165 69 26 210 69 30 stone',
  ];
  for (const c of cases) {
    if (c.isDripstone) {
      cmds.push(`setblock ${c.x} 205 28 stone`);
      cmds.push(`setblock ${c.x} 204 28 ${c.block}`);
    } else {
      cmds.push(`setblock ${c.x} 200 28 stone`);
      cmds.push(`setblock ${c.x} 201 28 ${c.block}`);
    }
  }
  // Remove support to trigger falling
  for (const c of cases) {
    if (c.isDripstone) {
      cmds.push(`setblock ${c.x} 205 28 air`);
    } else {
      cmds.push(`setblock ${c.x} 200 28 air`);
    }
  }
  return cmds;
}

const verify = cases.flatMap(({ name, x }) => [
  { name, command: `data get entity ${selector(x)} BlockState` },
  { name, command: `data get entity ${selector(x)} HurtEntities` },
  { name, command: `data get entity ${selector(x)} FallHurtAmount` },
  { name, command: `data get entity ${selector(x)} FallHurtMax` },
]);

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
    const setupCmds = buildSetup();

    client.on('position', () => {
      setTimeout(() => {
        setupCmds.forEach((cmd, idx) => {
          setTimeout(() => {
            client.write('chat_command', { command: cmd, timestamp: BigInt(Date.now()) });
          }, idx * 150);
        });

        const verifyStart = setupCmds.length * 150 + 200;
        verify.forEach(({ name: caseName, command }, idx) => {
          setTimeout(() => {
            client.write('chat_command', { command, timestamp: BigInt(Date.now()) });
          }, verifyStart + idx * 50);
        });

        const totalTime = verifyStart + verify.length * 50 + 1500;
        setTimeout(() => {
          client.end();
          resolve(log);
        }, totalTime);
      }, 500);
    });

    client.on('system_chat', packet => {
      const text = summarize(packet.content);
      if (text.includes('multiplayer.player.joined')) return;
      log.push(text);
    });

    client.on('error', reject);
  });
}

async function main() {
  console.log('--- Step 1: Testing Expanded Falling Blocks on Pumpkin (25565) ---');
  const pLog = await runServer(25565, 'PUMPKIN');
  console.log('--- Step 2: Testing Expanded Falling Blocks on Vanilla (25575) ---');
  const vLog = await runServer(25575, 'VANILLA');

  console.log('\n--- Step 3: Raw Verification Logs Sample ---');
  console.log('Pumpkin entries:', pLog.length, 'Vanilla entries:', vLog.length);
  pLog.slice(-verify.length).forEach((line, i) => {
    const vLine = vLog.slice(-verify.length)[i] || '<missing>';
    console.log(`[PUMPKIN] ${line}`);
    console.log(`[VANILLA] ${vLine}\n`);
  });
}

main().catch(err => {
  console.error('Fatal error:', err);
  process.exit(1);
});

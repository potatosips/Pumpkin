const mc = require('minecraft-protocol');

const cases = [
  { name: 'sand', x: 170, block: 'minecraft:sand' },
  { name: 'red_sand', x: 173, block: 'minecraft:red_sand' },
  { name: 'gravel', x: 176, block: 'minecraft:gravel' },
  { name: 'white_concrete_powder', x: 179, block: 'minecraft:white_concrete_powder' },
  { name: 'black_concrete_powder', x: 182, block: 'minecraft:black_concrete_powder' },
  { name: 'cyan_concrete_powder', x: 185, block: 'minecraft:cyan_concrete_powder' },
  { name: 'lime_concrete_powder', x: 188, block: 'minecraft:lime_concrete_powder' },
  { name: 'anvil_north', x: 191, block: 'minecraft:anvil[facing=north]' },
  { name: 'anvil_south', x: 194, block: 'minecraft:anvil[facing=south]' },
  { name: 'chipped_anvil_east', x: 197, block: 'minecraft:chipped_anvil[facing=east]' },
  { name: 'damaged_anvil_west', x: 200, block: 'minecraft:damaged_anvil[facing=west]' },
  { name: 'pointed_dripstone', x: 203, block: 'minecraft:pointed_dripstone[vertical_direction=down,thickness=tip,waterlogged=false]', isDripstone: true },
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
  for (const c of cases) {
    if (c.isDripstone) {
      cmds.push(`setblock ${c.x} 205 28 air`);
    } else {
      cmds.push(`setblock ${c.x} 200 28 air`);
    }
  }
  return cmds;
}

const setup = buildSetup();
const verify = cases.flatMap(({ name, x }) => [
  { name, command: `data get entity ${selector(x)} BlockState` },
  { name, command: `data get entity ${selector(x)} HurtEntities` },
  { name, command: `data get entity ${selector(x)} FallHurtAmount` },
  { name, command: `data get entity ${selector(x)} FallHurtMax` },
]);

let finished = 0;
function summarize(node) {
  if (node == null) return '';
  if (typeof node !== 'object') return String(node);
  if (node.type && node.type !== 'compound' && node.type !== 'list') return summarize(node.value);
  if (node.type === 'list') return (node.value?.value ?? node.value ?? []).map(summarize).join('');
  if (node.type === 'compound') return Object.values(node.value ?? {}).map(summarize).filter(Boolean).join('|');
  return Object.values(node).map(summarize).filter(Boolean).join('|');
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
      }, index * 200));

      const verifyStart = setup.length * 200 + 200;
      verify.forEach(({ name: caseName, command }, index) => setTimeout(() => {
        client.write('chat_command', { command, timestamp: BigInt(Date.now()) });
      }, verifyStart + index * 40));

      setTimeout(() => client.end(), verifyStart + verify.length * 40 + 1500);
    }, 500);
  });

  client.on('system_chat', packet => {
    const text = summarize(packet.content);
    if (text.includes('commands.data.entity.query')) {
      console.log(`[${name}] ${text}`);
    }
  });

  client.on('error', error => console.error(`[${name}] ERROR ${error.message}`));
  client.on('end', () => {
    if (++finished === 2) {
      console.log('\nBoth servers completed falling block differential sweep!');
      process.exit(0);
    }
  });
}

run('PUMPKIN', 25565);
run('VANILLA', 25575);

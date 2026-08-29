const mc = require('minecraft-protocol');

const phase = process.argv[2] ?? 'setup';
const tag = 'falling_persist_1';
const selector = `@e[type=falling_block,tag=${tag},limit=1]`;
const queries = [
  `data get entity ${selector} UUID`,
  `data get entity ${selector} Pos`,
  `data get entity ${selector} Motion`,
  `data get entity ${selector} BlockState`,
  `data get entity ${selector} Time`,
  `data get entity ${selector} DropItem`,
  `data get entity ${selector} HurtEntities`,
  `data get entity ${selector} FallHurtAmount`,
  `data get entity ${selector} FallHurtMax`,
  `data get entity ${selector} CancelDrop`,
  `data get entity ${selector} TileEntityData`,
  `data get entity ${selector} NoGravity`,
];

const commands = phase === 'setup' ? [
  `kill @e[tag=${tag}]`,
  `summon falling_block 4.5 80 28.5 {Tags:["${tag}"],NoGravity:1b,Motion:[0.0d,0.0d,0.0d],BlockState:{Name:"minecraft:oak_log",Properties:{axis:"x"}},Time:-1000,DropItem:0b,HurtEntities:1b,FallHurtAmount:3.5f,FallHurtMax:23,CancelDrop:1b,TileEntityData:{proof:"retained",value:7}}`,
  ...queries,
  'save-all flush',
  `teleport ${selector} 68.5 80 28.5`,
  'save-all flush',
  `data get entity ${selector} UUID`,
  `data get entity ${selector} Pos`,
] : [
  ...queries,
  `kill @e[type=falling_block,tag=${tag}]`,
];

let finished = 0;
function summarize(node) {
  if (node == null) return '';
  if (typeof node !== 'object') return String(node);
  if (node.type && node.type !== 'compound' && node.type !== 'list') return summarize(node.value);
  if (node.type === 'list') return (node.value?.value ?? node.value ?? []).map(summarize).join('');
  if (node.type === 'compound') {
    const value = node.value ?? {};
    const preferred = ['translate', 'text', 'with', 'extra'];
    return [...preferred.map(key => value[key]),
      ...Object.entries(value).filter(([key]) => !preferred.includes(key)).map(([, child]) => child)]
      .map(summarize).filter(Boolean).join('|');
  }
  return Object.values(node).map(summarize).filter(Boolean).join('|');
}

function run(name, port) {
  const client = mc.createClient({host: '127.0.0.1', port, username: 'TestBot', version: '1.21.4', auth: 'offline'});
  let sent = false;
  client.on('position', () => {
    if (sent) return;
    sent = true;
    setTimeout(() => {
      commands.forEach((command, index) => setTimeout(() => {
        console.log(`[${name}] > ${command}`);
        client.write('chat_command', {command, timestamp: BigInt(Date.now())});
      }, index * 850));
      setTimeout(() => client.end(), commands.length * 850 + 1800);
    }, 1200);
  });
  client.on('system_chat', packet => console.log(`[${name}] < ${summarize(packet.content)}`));
  client.on('error', error => console.error(`[${name}] ERROR ${error.message}`));
  client.on('end', () => { if (++finished === 2) process.exit(0); });
}

run('PUMPKIN', 25565);
run('VANILLA', 25575);

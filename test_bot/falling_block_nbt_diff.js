const mc = require('minecraft-protocol');

const tag = 'falling_nbt_defaults_1';
const target = `@e[type=falling_block,tag=${tag},limit=1]`;
const commands = [
  `kill @e[tag=${tag}]`,
  `summon falling_block 20 80 20 {Tags:["${tag}"],NoGravity:1b,BlockState:{Name:"minecraft:stone"}}`,
  `data get entity ${target} BlockState`,
  `data get entity ${target} Time`,
  `data get entity ${target} DropItem`,
  `data get entity ${target} HurtEntities`,
  `data get entity ${target} FallHurtAmount`,
  `data get entity ${target} FallHurtMax`,
  `data get entity ${target} CancelDrop`,
  `data get entity ${target} Pos`,
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
      }, index * 900));
      setTimeout(() => client.end(), commands.length * 900 + 1800);
    }, 1200);
  });
  client.on('system_chat', packet => console.log(`[${name}] < ${summarize(packet.content)}`));
  client.on('error', error => console.error(`[${name}] ERROR ${error.message}`));
  client.on('end', () => { if (++finished === 2) process.exit(0); });
}

run('PUMPKIN', 25565);
run('VANILLA', 25575);

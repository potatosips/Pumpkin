const mc = require('minecraft-protocol');

const tag = 'falling_edge_1';
const setup = [
  `kill @e[tag=${tag}]`,
  'kill @e[type=item,x=44,y=55,z=18,dx=32,dy=35,dz=4]',
  `summon falling_block 46.5 80 20.5 {Tags:["${tag}","${tag}_invalid"],NoGravity:1b,BlockState:{Name:"minecraft:oak_log",Properties:{axis:"invalid"}},Time:1,DropItem:0b}`,
  `data get entity @e[type=falling_block,tag=${tag}_invalid,limit=1] BlockState`,
  `summon falling_block 50.5 80 20.5 {Tags:["${tag}","${tag}_nodrop"],NoGravity:1b,BlockState:{Name:"minecraft:stone"},Time:599,DropItem:0b}`,
  `summon falling_block 54.5 80 20.5 {Tags:["${tag}","${tag}_drop"],NoGravity:1b,BlockState:{Name:"minecraft:diamond_block"},Time:599,DropItem:1b}`,
  `summon falling_block 74.5 80 20.5 {Tags:["${tag}","${tag}_cancel"],NoGravity:1b,BlockState:{Name:"minecraft:emerald_block"},Time:599,DropItem:1b,CancelDrop:1b}`,
];

const verify = [
  `data get entity @e[type=falling_block,tag=${tag}_nodrop,limit=1] Pos`,
  `data get entity @e[type=falling_block,tag=${tag}_drop,limit=1] Pos`,
  `data get entity @e[type=falling_block,tag=${tag}_cancel,limit=1] Pos`,
  'data get entity @e[type=item,x=52,y=-64,z=18,dx=5,dy=384,dz=5,limit=1,sort=nearest] Item.id',
  'data get entity @e[type=item,x=72,y=-64,z=18,dx=5,dy=384,dz=5,limit=1,sort=nearest] Item.id',
  'data get entity @e[type=item,x=60,y=-64,z=10,dx=30,dy=384,dz=20,limit=1,sort=nearest] Item.id',
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
      setup.forEach((command, index) => setTimeout(() => {
        console.log(`[${name}] > ${command}`);
        client.write('chat_command', {command, timestamp: BigInt(Date.now())});
      }, index * 800));
      const verifyStart = setup.length * 800 + 2200;
      verify.forEach((command, index) => setTimeout(() => {
        console.log(`[${name}] > ${command}`);
        client.write('chat_command', {command, timestamp: BigInt(Date.now())});
      }, verifyStart + index * 800));
      setTimeout(() => client.end(), verifyStart + verify.length * 800 + 1600);
    }, 1200);
  });
  client.on('system_chat', packet => console.log(`[${name}] < ${summarize(packet.content)}`));
  client.on('error', error => console.error(`[${name}] ERROR ${error.message}`));
  client.on('end', () => { if (++finished === 2) process.exit(0); });
}

run('PUMPKIN', 25565);
run('VANILLA', 25575);

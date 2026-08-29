const mc = require('minecraft-protocol');

const tag = 'falling_height_1';
const commands = [
  `kill @e[tag=${tag}]`,
  'kill @e[type=item,x=208,y=50,z=26,dx=26,dy=300,dz=5]',
  'gamerule doEntityDrops true',
  `summon falling_block 210.5 -64 28.5 {Tags:["${tag}","${tag}_bottom"],NoGravity:1b,BlockState:{Name:"minecraft:stone"},Time:100,DropItem:0b}`,
  `summon falling_block 214.5 -63 28.5 {Tags:["${tag}","${tag}_inside"],NoGravity:1b,BlockState:{Name:"minecraft:stone"},Time:100,DropItem:0b}`,
  `summon falling_block 218.5 320 28.5 {Tags:["${tag}","${tag}_above_drop"],NoGravity:1b,BlockState:{Name:"minecraft:emerald_block"},Time:100,DropItem:1b}`,
  `summon falling_block 222.5 100 28.5 {Tags:["${tag}","${tag}_age_drop"],NoGravity:1b,BlockState:{Name:"minecraft:diamond_block"},Time:600,DropItem:1b}`,
  'gamerule doEntityDrops false',
  `summon falling_block 226.5 320 28.5 {Tags:["${tag}","${tag}_rule_nodrop"],NoGravity:1b,BlockState:{Name:"minecraft:lapis_block"},Time:100,DropItem:1b}`,
  'gamerule doEntityDrops true',
];

const verify = [
  `data get entity @e[tag=${tag}_bottom,limit=1] Time`,
  `data get entity @e[tag=${tag}_inside,limit=1] Time`,
  `data get entity @e[tag=${tag}_above_drop,limit=1] Time`,
  `data get entity @e[tag=${tag}_age_drop,limit=1] Time`,
  `data get entity @e[tag=${tag}_rule_nodrop,limit=1] Time`,
  'data get entity @e[type=item,x=216,y=-64,z=26,dx=5,dy=448,dz=5,limit=1] Item.id',
  'data get entity @e[type=item,x=220,y=-64,z=26,dx=5,dy=448,dz=5,limit=1] Item.id',
  'data get entity @e[type=item,x=224,y=-64,z=26,dx=5,dy=448,dz=5,limit=1] Item.id',
];

let finished = 0;
function summarize(node) {
  if (node == null) return '';
  if (typeof node !== 'object') return String(node);
  if (node.type && node.type !== 'compound' && node.type !== 'list') return summarize(node.value);
  if (node.type === 'list') return (node.value?.value ?? node.value ?? []).map(summarize).join('');
  return Object.values(node.value ?? node).map(summarize).filter(Boolean).join('|');
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
      }, index * 350));
      const verifyStart = commands.length * 350 + 1200;
      verify.forEach((command, index) => setTimeout(() => {
        console.log(`[${name}] VERIFY > ${command}`);
        client.write('chat_command', {command, timestamp: BigInt(Date.now())});
      }, verifyStart + index * 450));
      setTimeout(() => client.end(), verifyStart + verify.length * 450 + 1200);
    }, 900);
  });
  client.on('system_chat', packet => console.log(`[${name}] < ${summarize(packet.content)}`));
  client.on('error', error => console.error(`[${name}] ERROR ${error.message}`));
  client.on('end', () => { if (++finished === 2) process.exit(0); });
}

run('PUMPKIN', 25565);
run('VANILLA', 25575);

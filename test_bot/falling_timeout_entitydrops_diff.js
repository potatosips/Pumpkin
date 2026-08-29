const mc = require('minecraft-protocol');

const tag = 'falling_rule_1';
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
  let started = false;
  const send = command => {
    console.log(`[${name}] > ${command}`);
    client.write('chat_command', {command, timestamp: BigInt(Date.now())});
  };
  client.on('position', () => {
    if (started) return;
    started = true;
    setTimeout(() => send(`kill @e[tag=${tag}]`), 500);
    setTimeout(() => send('kill @e[type=item,x=238,y=150,z=26,dx=10,dy=80,dz=5]'), 850);
    setTimeout(() => send('fill 238 190 26 248 210 30 air'), 1200);
    setTimeout(() => send('gamerule doEntityDrops true'), 1550);
    setTimeout(() => send(`summon falling_block 240.5 200 28.5 {Tags:["${tag}","${tag}_true"],NoGravity:1b,BlockState:{Name:"minecraft:diamond_block"},Time:600,DropItem:1b}`), 1900);
    setTimeout(() => send(`data get entity @e[tag=${tag}_true,limit=1] Time`), 2600);
    setTimeout(() => send('data get entity @e[type=item,x=239,y=150,z=27,dx=3,dy=70,dz=3,limit=1] Item.id'), 3000);
    setTimeout(() => send('gamerule doEntityDrops false'), 3450);
    setTimeout(() => send(`summon falling_block 246.5 200 28.5 {Tags:["${tag}","${tag}_false"],NoGravity:1b,BlockState:{Name:"minecraft:emerald_block"},Time:600,DropItem:1b}`), 3800);
    setTimeout(() => send(`data get entity @e[tag=${tag}_false,limit=1] Time`), 4500);
    setTimeout(() => send('data get entity @e[type=item,x=245,y=150,z=27,dx=3,dy=70,dz=3,limit=1] Item.id'), 4900);
    setTimeout(() => send('gamerule doEntityDrops true'), 5300);
    setTimeout(() => client.end(), 6500);
  });
  client.on('system_chat', packet => console.log(`[${name}] < ${summarize(packet.content)}`));
  client.on('error', error => console.error(`[${name}] ERROR ${error.message}`));
  client.on('end', () => { if (++finished === 2) process.exit(0); });
}

run('PUMPKIN', 25565);
run('VANILLA', 25575);

const mc = require('minecraft-protocol');

const tag = 'no_gravity_families_1';
const commands = [
  `kill @e[tag=${tag}]`,
  `summon item 4 80 12 {Tags:["${tag}","${tag}_item"],NoGravity:1b,Item:{id:"minecraft:stone",count:1}}`,
  `data get entity @e[type=item,tag=${tag}_item,limit=1] Pos`,
  `summon falling_block 6 80 12 {Tags:["${tag}","${tag}_falling"],NoGravity:1b,BlockState:{Name:"minecraft:stone"},Time:1,DropItem:0b}`,
  `data get entity @e[type=falling_block,tag=${tag}_falling,limit=1] Pos`,
  `summon experience_orb 8 80 12 {Tags:["${tag}","${tag}_orb"],NoGravity:1b,Value:1}`,
  `data get entity @e[type=experience_orb,tag=${tag}_orb,limit=1] Pos`,
  `summon arrow 10 80 12 {Tags:["${tag}","${tag}_arrow"],NoGravity:1b,Motion:[0.0d,0.0d,0.0d]}`,
  `data get entity @e[type=arrow,tag=${tag}_arrow,limit=1] Pos`,
  `summon minecart 12 80 12 {Tags:["${tag}","${tag}_minecart"],NoGravity:1b}`,
  `data get entity @e[type=minecart,tag=${tag}_minecart,limit=1] Pos`,
  `summon tnt 14 80 12 {Tags:["${tag}","${tag}_tnt"],NoGravity:1b,fuse:200s}`,
  `data get entity @e[type=tnt,tag=${tag}_tnt,limit=1] Pos`,
  `summon item 16 80 12 {Tags:["${tag}","${tag}_control"],Item:{id:"minecraft:stone",count:1}}`,
  `data get entity @e[type=item,tag=${tag}_control,limit=1] Pos`,
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
      }, index * 1000));
      setTimeout(() => client.end(), commands.length * 1000 + 1800);
    }, 1200);
  });
  client.on('system_chat', packet => console.log(`[${name}] < ${summarize(packet.content)}`));
  client.on('error', error => console.error(`[${name}] ERROR ${error.message}`));
  client.on('end', () => { if (++finished === 2) process.exit(0); });
}

run('PUMPKIN', 25565);
run('VANILLA', 25575);

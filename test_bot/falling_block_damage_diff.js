const mc = require('minecraft-protocol');

const tag = 'falling_damage_1';
const setup = [
  `kill @e[tag=${tag}]`,
  'fill 88 70 26 100 84 30 air',
  'fill 88 69 26 100 69 30 stone',
  `summon cow 90.5 70 28.5 {Tags:["${tag}","${tag}_normal"],Health:10.0f,NoAI:1b,NoGravity:1b,PersistenceRequired:1b}`,
  `summon cow 94.5 70 28.5 {Tags:["${tag}","${tag}_control"],Health:10.0f,NoAI:1b,NoGravity:1b,PersistenceRequired:1b}`,
  `summon cow 98.5 70 28.5 {Tags:["${tag}","${tag}_cap"],Health:10.0f,NoAI:1b,NoGravity:1b,PersistenceRequired:1b}`,
  `summon falling_block 90.5 80 28.5 {Tags:["${tag}"],BlockState:{Name:"minecraft:stone"},HurtEntities:1b,FallHurtAmount:0.5f,FallHurtMax:40,DropItem:0b,CancelDrop:1b}`,
  `summon falling_block 94.5 80 28.5 {Tags:["${tag}"],BlockState:{Name:"minecraft:stone"},HurtEntities:0b,FallHurtAmount:2.0f,FallHurtMax:40,DropItem:0b,CancelDrop:1b}`,
  `summon falling_block 98.5 80 28.5 {Tags:["${tag}"],BlockState:{Name:"minecraft:stone"},HurtEntities:1b,FallHurtAmount:10.0f,FallHurtMax:3,DropItem:0b,CancelDrop:1b}`,
];
const verify = [
  `data get entity @e[type=cow,tag=${tag}_normal,limit=1] Health`,
  `data get entity @e[type=cow,tag=${tag}_control,limit=1] Health`,
  `data get entity @e[type=cow,tag=${tag}_cap,limit=1] Health`,
  `data get entity @e[type=falling_block,tag=${tag},limit=1] Pos`,
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
      }, index * 700));
      const verifyStart = setup.length * 700 + 5000;
      verify.forEach((command, index) => setTimeout(() => {
        console.log(`[${name}] > ${command}`);
        client.write('chat_command', {command, timestamp: BigInt(Date.now())});
      }, verifyStart + index * 800));
      setTimeout(() => client.end(), verifyStart + verify.length * 800 + 1800);
    }, 1200);
  });
  client.on('system_chat', packet => console.log(`[${name}] < ${summarize(packet.content)}`));
  client.on('error', error => console.error(`[${name}] ERROR ${error.message}`));
  client.on('end', () => { if (++finished === 2) process.exit(0); });
}

run('PUMPKIN', 25565);
run('VANILLA', 25575);

const mc = require('minecraft-protocol');

const tag = 'falling_land_1';
const setup = [
  `kill @e[tag=${tag}]`,
  `kill @e[type=item,x=28,y=68,z=18,dx=12,dy=8,dz=4]`,
  'fill 28 70 18 40 78 22 air',
  'fill 28 69 18 40 69 22 stone',
  `summon falling_block 30.5 76 20.5 {Tags:["${tag}","${tag}_place"],BlockState:{Name:"minecraft:oak_log",Properties:{axis:"x"}},Time:1,DropItem:1b}`,
  `summon falling_block 34.5 76 20.5 {Tags:["${tag}","${tag}_cancel"],BlockState:{Name:"minecraft:stone"},Time:1,DropItem:1b,CancelDrop:1b}`,
  `summon falling_block 38.5 76 20.5 {Tags:["${tag}","${tag}_drop"],BlockState:{Name:"minecraft:cactus"},Time:1,DropItem:1b}`,
];

const verify = [
  'fill 30 70 20 30 70 20 gold_block keep',
  'fill 34 70 20 34 70 20 gold_block keep',
  `data get entity @e[type=falling_block,tag=${tag}_cancel,limit=1] Pos`,
  'fill 38 70 20 38 70 20 gold_block keep',
  `data get entity @e[type=item,x=38.5,y=70.5,z=20.5,distance=..2,limit=1] Item.id`,
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
      }, verifyStart + index * 700));
      setTimeout(() => client.end(), verifyStart + verify.length * 700 + 1800);
    }, 1200);
  });
  client.on('system_chat', packet => console.log(`[${name}] < ${summarize(packet.content)}`));
  client.on('error', error => console.error(`[${name}] ERROR ${error.message}`));
  client.on('end', () => { if (++finished === 2) process.exit(0); });
}

run('PUMPKIN', 25565);
run('VANILLA', 25575);

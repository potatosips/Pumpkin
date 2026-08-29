const mc = require('minecraft-protocol');

const tag = 'falling_tile_data_1';
const verifyOnly = process.env.VERIFY_ONLY === '1';
const tileData = 'TileEntityData:{id:"minecraft:chest",x:999,y:-999,z:999,CustomName:\'"Parity Barrel"\',Lock:"falling-proof",Items:[{Slot:0b,id:"minecraft:diamond",count:3}]}' ;
const setup = [
  'tp @s 164 80 28',
  `kill @e[tag=${tag}]`,
  'fill 158 70 26 170 84 30 air',
  'fill 158 69 26 170 69 30 stone',
  `summon falling_block 160.5 76 28.5 {Tags:["${tag}"],BlockState:{Name:"minecraft:barrel",Properties:{facing:"up",open:"false"}},DropItem:0b,${tileData}}`,
  `summon falling_block 164.5 76 28.5 {Tags:["${tag}"],BlockState:{Name:"minecraft:barrel",Properties:{facing:"down",open:"false"}},DropItem:0b}`,
  `summon falling_block 168.5 76 28.5 {Tags:["${tag}"],BlockState:{Name:"minecraft:barrel",Properties:{facing:"north",open:"false"}},DropItem:0b,CancelDrop:1b,${tileData}}`,
];
const verify = [
  'data get block 160 70 28 id',
  'data get block 160 70 28 x',
  'data get block 160 70 28 y',
  'data get block 160 70 28 z',
  'data get block 160 70 28 CustomName',
  'data get block 160 70 28 Lock',
  'data get block 160 70 28 Items[0].id',
  'data get block 160 70 28 Items[0].count',
  'data get block 164 70 28 Items',
  'data get block 168 70 28',
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
      const commands = verifyOnly ? [] : setup;
      commands.forEach((command, index) => setTimeout(() => {
        console.log(`[${name}] > ${command}`);
        client.write('chat_command', {command, timestamp: BigInt(Date.now())});
      }, index * 800));
      const verifyStart = commands.length * 800 + (verifyOnly ? 1500 : 6000);
      verify.forEach((command, index) => setTimeout(() => {
        console.log(`[${name}] > ${command}`);
        client.write('chat_command', {command, timestamp: BigInt(Date.now())});
      }, verifyStart + index * 850));
      setTimeout(() => client.end(), verifyStart + verify.length * 850 + 1800);
    }, 1200);
  });
  client.on('system_chat', packet => console.log(`[${name}] < ${summarize(packet.content)}`));
  client.on('error', error => console.error(`[${name}] ERROR ${error.message}`));
  client.on('end', () => { if (++finished === 2) process.exit(0); });
}

run('PUMPKIN', 25565);
run('VANILLA', 25575);

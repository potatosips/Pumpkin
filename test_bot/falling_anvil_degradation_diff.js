const mc = require('minecraft-protocol');
const mcData = require('minecraft-data')('1.21.4');

const tag = 'falling_anvil_degrade_1';
const setup = [
  'tp @s 120 80 28',
  `kill @e[tag=${tag}]`,
  'fill 108 70 26 132 112 30 air',
  'fill 108 69 26 132 69 30 stone',
  `summon cow 110.5 70 28.5 {Tags:["${tag}","${tag}_anvil_cow"],Health:10.0f,NoAI:1b,NoGravity:1b,PersistenceRequired:1b}`,
  `summon cow 118.5 70 28.5 {Tags:["${tag}","${tag}_chipped_cow"],Health:10.0f,NoAI:1b,NoGravity:1b,PersistenceRequired:1b}`,
  `summon cow 122.5 70 28.5 {Tags:["${tag}","${tag}_damaged_cow"],Health:10.0f,NoAI:1b,NoGravity:1b,PersistenceRequired:1b}`,
  `summon falling_block 110.5 105 28.5 {Tags:["${tag}"],BlockState:{Name:"minecraft:anvil",Properties:{facing:"north"}},HurtEntities:1b,FallHurtAmount:0.1f,FallHurtMax:1,DropItem:0b}`,
  `summon falling_block 114.5 105 28.5 {Tags:["${tag}"],BlockState:{Name:"minecraft:anvil",Properties:{facing:"east"}},HurtEntities:1b,FallHurtAmount:0.1f,FallHurtMax:1,DropItem:0b}`,
  `summon falling_block 118.5 105 28.5 {Tags:["${tag}"],BlockState:{Name:"minecraft:chipped_anvil",Properties:{facing:"south"}},HurtEntities:1b,FallHurtAmount:0.1f,FallHurtMax:1,DropItem:0b}`,
  `summon falling_block 122.5 105 28.5 {Tags:["${tag}"],BlockState:{Name:"minecraft:damaged_anvil",Properties:{facing:"west"}},HurtEntities:1b,FallHurtAmount:0.1f,FallHurtMax:1,DropItem:0b}`,
  `summon falling_block 126.5 105 28.5 {Tags:["${tag}"],BlockState:{Name:"minecraft:anvil",Properties:{facing:"south"}},HurtEntities:1b,FallHurtAmount:0.0f,FallHurtMax:0,DropItem:0b}`,
  `summon falling_block 130.5 105 28.5 {Tags:["${tag}"],BlockState:{Name:"minecraft:anvil",Properties:{facing:"west"}},HurtEntities:0b,FallHurtAmount:2.0f,FallHurtMax:40,DropItem:0b}`,
];

const verify = [
  `execute if block 110 70 28 minecraft:chipped_anvil[facing=north]`,
  `execute if block 114 70 28 minecraft:chipped_anvil[facing=east]`,
  `execute if block 118 70 28 minecraft:damaged_anvil[facing=south]`,
  `execute if block 122 70 28 minecraft:air`,
  `execute if block 126 70 28 minecraft:chipped_anvil[facing=south]`,
  `execute if block 130 70 28 minecraft:anvil[facing=west]`,
  `data get entity @e[type=cow,tag=${tag}_anvil_cow,limit=1] Health`,
  `data get entity @e[type=cow,tag=${tag}_chipped_cow,limit=1] Health`,
  `data get entity @e[type=cow,tag=${tag}_damaged_cow,limit=1] Health`,
  `data get entity @e[type=falling_block,tag=${tag},limit=1] Pos`,
  `data get entity @e[type=item,nbt={Item:{id:"minecraft:damaged_anvil"}},distance=..200,limit=1] Item.id`,
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
      const verifyStart = setup.length * 700 + 9000;
      verify.forEach((command, index) => setTimeout(() => {
        console.log(`[${name}] > ${command}`);
        client.write('chat_command', {command, timestamp: BigInt(Date.now())});
      }, verifyStart + index * 800));
      setTimeout(() => client.end(), verifyStart + verify.length * 800 + 1800);
    }, 1200);
  });
  client.on('system_chat', packet => console.log(`[${name}] < ${summarize(packet.content)}`));
  client.on('block_change', packet => {
    const {x, y, z} = packet.location;
    if (y !== 70 || z !== 28 || ![110, 114, 118, 122, 126, 130].includes(x)) return;
    const block = mcData.blocksByStateId[packet.type];
    const propertyIndex = block ? packet.type - block.minStateId : -1;
    const facing = block?.states?.[0]?.values?.[propertyIndex];
    console.log(`[${name}] BLOCK ${x} ${y} ${z} state=${packet.type} ${block?.name ?? 'unknown'}${facing ? `[facing=${facing}]` : ''}`);
  });
  client.on('error', error => console.error(`[${name}] ERROR ${error.message}`));
  client.on('end', () => { if (++finished === 2) process.exit(0); });
}

run('PUMPKIN', 25565);
run('VANILLA', 25575);

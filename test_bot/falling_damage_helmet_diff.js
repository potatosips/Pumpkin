const mc = require('minecraft-protocol');

const tag = 'falling_helmet_1';
const ironHelmet = '{id:"minecraft:iron_helmet",count:1,components:{"minecraft:damage":0}}';
const ironBoots = '{id:"minecraft:iron_boots",count:1,components:{"minecraft:damage":0}}';
const empty = '{}';
const setup = [
  'tp @s 146 80 28',
  `kill @e[tag=${tag}]`,
  'fill 138 70 26 154 84 30 air',
  'fill 138 69 26 154 69 30 stone',
  `summon husk 140.5 70 28.5 {Tags:["${tag}","${tag}_anvil_none"],Health:20.0f,NoAI:1b,NoGravity:1b,PersistenceRequired:1b}`,
  `summon husk 144.5 70 28.5 {Tags:["${tag}","${tag}_anvil_helmet"],Health:20.0f,NoAI:1b,NoGravity:1b,PersistenceRequired:1b,ArmorItems:[${empty},${empty},${empty},${ironHelmet}]}`,
  `summon husk 148.5 70 28.5 {Tags:["${tag}","${tag}_anvil_boots"],Health:20.0f,NoAI:1b,NoGravity:1b,PersistenceRequired:1b,ArmorItems:[${ironBoots},${empty},${empty},${empty}]}`,
  `summon husk 152.5 70 28.5 {Tags:["${tag}","${tag}_stone_helmet"],Health:20.0f,NoAI:1b,NoGravity:1b,PersistenceRequired:1b,ArmorItems:[${empty},${empty},${empty},${ironHelmet}]}`,
  `summon falling_block 140.5 80 28.5 {Tags:["${tag}"],BlockState:{Name:"minecraft:anvil"},HurtEntities:1b,FallHurtAmount:1.0f,FallHurtMax:40,DropItem:0b,CancelDrop:1b}`,
  `summon falling_block 144.5 80 28.5 {Tags:["${tag}"],BlockState:{Name:"minecraft:anvil"},HurtEntities:1b,FallHurtAmount:1.0f,FallHurtMax:40,DropItem:0b,CancelDrop:1b}`,
  `summon falling_block 148.5 80 28.5 {Tags:["${tag}"],BlockState:{Name:"minecraft:anvil"},HurtEntities:1b,FallHurtAmount:1.0f,FallHurtMax:40,DropItem:0b,CancelDrop:1b}`,
  `summon falling_block 152.5 80 28.5 {Tags:["${tag}"],BlockState:{Name:"minecraft:stone"},HurtEntities:1b,FallHurtAmount:1.0f,FallHurtMax:40,DropItem:0b,CancelDrop:1b}`,
];
const verify = [
  `data get entity @e[type=husk,tag=${tag}_anvil_none,limit=1] Health`,
  `data get entity @e[type=husk,tag=${tag}_anvil_helmet,limit=1] Health`,
  `data get entity @e[type=husk,tag=${tag}_anvil_boots,limit=1] Health`,
  `data get entity @e[type=husk,tag=${tag}_stone_helmet,limit=1] Health`,
  `data get entity @e[type=husk,tag=${tag}_anvil_helmet,limit=1] ArmorItems`,
  `data get entity @e[type=husk,tag=${tag}_anvil_boots,limit=1] ArmorItems`,
  `data get entity @e[type=husk,tag=${tag}_stone_helmet,limit=1] ArmorItems`,
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
      const verifyStart = setup.length * 700 + 6000;
      verify.forEach((command, index) => setTimeout(() => {
        console.log(`[${name}] > ${command}`);
        client.write('chat_command', {command, timestamp: BigInt(Date.now())});
      }, verifyStart + index * 900));
      setTimeout(() => client.end(), verifyStart + verify.length * 900 + 1800);
    }, 1200);
  });
  client.on('system_chat', packet => console.log(`[${name}] < ${summarize(packet.content)}`));
  client.on('error', error => console.error(`[${name}] ERROR ${error.message}`));
  client.on('end', () => { if (++finished === 2) process.exit(0); });
}

run('PUMPKIN', 25565);
run('VANILLA', 25575);

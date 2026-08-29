const mc = require('minecraft-protocol');

const commands = [
  'kill @e[tag=mob_flags_3]',
  'summon zombie 4 80 4 {Tags:["mob_flags_3"],PersistenceRequired:1b,NoAI:1b,NoGravity:1b,LeftHanded:1b,CanPickUpLoot:1b}',
  'summon bat 6 80 4 {Tags:["mob_flags_3"],PersistenceRequired:1b,NoAI:1b,NoGravity:1b,LeftHanded:1b,CanPickUpLoot:1b}',
  'summon slime 8 80 4 {Tags:["mob_flags_3"],PersistenceRequired:1b,NoAI:1b,NoGravity:1b,LeftHanded:1b,CanPickUpLoot:1b,Size:1}',
  'data get entity @e[type=zombie,tag=mob_flags_3,limit=1] NoAI',
  'data get entity @e[type=zombie,tag=mob_flags_3,limit=1] NoGravity',
  'data get entity @e[type=zombie,tag=mob_flags_3,limit=1] LeftHanded',
  'data get entity @e[type=zombie,tag=mob_flags_3,limit=1] CanPickUpLoot',
  'data get entity @e[type=zombie,tag=mob_flags_3,limit=1] PersistenceRequired',
  'data get entity @e[type=bat,tag=mob_flags_3,limit=1] NoAI',
  'data get entity @e[type=bat,tag=mob_flags_3,limit=1] LeftHanded',
  'data get entity @e[type=slime,tag=mob_flags_3,limit=1] NoAI',
  'data get entity @e[type=slime,tag=mob_flags_3,limit=1] CanPickUpLoot',
  'data remove entity @e[type=zombie,tag=mob_flags_3,limit=1] NoAI',
  'data get entity @e[type=zombie,tag=mob_flags_3,limit=1] NoAI',
  'data get entity @e[tag=mob_flags_missing,limit=1] NoAI',
];

let finished = 0;
function summarize(node) {
  if (node == null) return '';
  if (typeof node !== 'object') return String(node);
  if (node.type && node.type !== 'compound' && node.type !== 'list') {
    return summarize(node.value);
  }
  if (node.type === 'list') {
    const values = node.value?.value ?? node.value ?? [];
    return values.map(summarize).filter(Boolean).join('');
  }
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
  const send = command => {
    console.log(`[${name}] > ${command}`);
    client.write('chat_command', {command, timestamp: BigInt(Date.now())});
  };
  client.on('position', () => {
    if (sent) return;
    sent = true;
    setTimeout(() => {
      commands.forEach((command, index) => setTimeout(() => send(command), index * 1000));
      setTimeout(() => client.end(), commands.length * 1000 + 1500);
    }, 1500);
  });
  client.on('system_chat', packet => console.log(`[${name}] < ${summarize(packet.content)}`));
  client.on('error', error => console.error(`[${name}] ERROR ${error.message}`));
  client.on('end', () => { if (++finished === 2) process.exit(0); });
}
run('PUMPKIN', 25565);
run('VANILLA', 25575);

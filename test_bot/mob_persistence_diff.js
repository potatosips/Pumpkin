const mc = require('minecraft-protocol');

const phase = process.argv[2] ?? 'setup';
const tag = 'mob_persist_1';
const selectors = {
  yes: `@e[type=cow,tag=${tag}_yes,limit=1]`,
  no: `@e[type=cow,tag=${tag}_no,limit=1]`,
  absent: `@e[type=cow,tag=${tag}_absent,limit=1]`,
};

const queries = Object.entries(selectors).flatMap(([label, selector]) => [
  `data get entity ${selector} NoAI`,
  `data get entity ${selector} LeftHanded`,
  `data get entity ${selector} CanPickUpLoot`,
  `data get entity ${selector} PersistenceRequired`,
]);

const commands = phase === 'setup' ? [
  `kill @e[tag=${tag}]`,
  `summon cow 12 80 4 {Tags:["${tag}","${tag}_yes"],NoGravity:1b,NoAI:1b,LeftHanded:1b,CanPickUpLoot:1b,PersistenceRequired:1b}`,
  `summon cow 14 80 4 {Tags:["${tag}","${tag}_no"],NoGravity:1b,NoAI:0b,LeftHanded:0b,CanPickUpLoot:0b,PersistenceRequired:1b}`,
  `summon cow 16 80 4 {Tags:["${tag}","${tag}_absent"],NoGravity:1b,PersistenceRequired:1b}`,
  ...queries,
] : queries;

let finished = 0;
function summarize(node) {
  if (node == null) return '';
  if (typeof node !== 'object') return String(node);
  if (node.type && node.type !== 'compound' && node.type !== 'list') return summarize(node.value);
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
      commands.forEach((command, index) => setTimeout(() => send(command), index * 900));
      setTimeout(() => client.end(), commands.length * 900 + 1500);
    }, 1200);
  });
  client.on('system_chat', packet => console.log(`[${name}] < ${summarize(packet.content)}`));
  client.on('error', error => console.error(`[${name}] ERROR ${error.message}`));
  client.on('end', () => { if (++finished === 2) process.exit(0); });
}

run('PUMPKIN', 25565);
run('VANILLA', 25575);

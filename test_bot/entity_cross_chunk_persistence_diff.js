const mc = require('minecraft-protocol');

const phase = process.argv[2] ?? 'setup';
const tag = 'cross_chunk_save_1';
const selector = `@e[type=cow,tag=${tag},limit=1]`;
const commands = phase === 'setup' ? [
  `kill @e[tag=${tag}]`,
  `summon cow 4 80 4 {Tags:["${tag}"],NoAI:1b,NoGravity:1b,PersistenceRequired:1b}`,
  'save-all flush',
  `teleport ${selector} 68 80 4`,
  'save-all flush',
] : [
  `data get entity ${selector} Pos`,
  `data get entity ${selector} UUID`,
  `kill @e[type=cow,tag=${tag}]`,
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
      }, index * 1300));
      setTimeout(() => client.end(), commands.length * 1300 + 1800);
    }, 1200);
  });
  client.on('system_chat', packet => console.log(`[${name}] < ${summarize(packet.content)}`));
  client.on('error', error => console.error(`[${name}] ERROR ${error.message}`));
  client.on('end', () => { if (++finished === 2) process.exit(0); });
}

run('PUMPKIN', 25565);
run('VANILLA', 25575);

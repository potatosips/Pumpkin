const mc = require('minecraft-protocol');

const phase = process.argv[2] ?? 'setup';
const selector = '@e[type=cow,tag=snapshot_delete_1,limit=1]';
const commands = phase === 'setup' ? [
  'kill @e[tag=snapshot_delete_1]',
  'summon cow 20 80 4 {Tags:["snapshot_delete_1"],NoGravity:1b,PersistenceRequired:1b}',
  'save-all flush',
  `kill ${selector}`,
] : [`data get entity ${selector} UUID`];

let finished = 0;
function translation(node) {
  if (node == null) return '';
  if (typeof node !== 'object') return String(node);
  if (node.type && node.type !== 'compound' && node.type !== 'list') return translation(node.value);
  if (node.type === 'list') return (node.value?.value ?? node.value ?? []).map(translation).join('');
  if (node.type === 'compound') {
    const value = node.value ?? {};
    return [value.translate, value.text, value.with, value.extra].map(translation).filter(Boolean).join('|');
  }
  return Object.values(node).map(translation).filter(Boolean).join('|');
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
      }, index * 1200));
      setTimeout(() => client.end(), commands.length * 1200 + 1800);
    }, 1200);
  });
  client.on('system_chat', packet => console.log(`[${name}] < ${translation(packet.content)}`));
  client.on('error', error => console.error(`[${name}] ERROR ${error.message}`));
  client.on('end', () => { if (++finished === 2) process.exit(0); });
}

run('PUMPKIN', 25565);
run('VANILLA', 25575);

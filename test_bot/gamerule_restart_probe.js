const mc = require('minecraft-protocol');
const mode = process.env.MODE || 'query';
const commands = mode === 'set'
  ? ['gamerule fallDamage false', 'gamerule randomTickSpeed -42', 'gamerule disableRaids true', 'gamerule doFireTick false', 'save-all flush']
  : mode === 'restore'
    ? ['gamerule fallDamage true', 'gamerule randomTickSpeed 3', 'gamerule disableRaids false', 'gamerule doFireTick true', 'save-all flush']
    : ['gamerule fallDamage', 'gamerule randomTickSpeed', 'gamerule disableRaids', 'gamerule doFireTick'];

function flatten(node) {
  if (node == null) return '';
  if (typeof node !== 'object') return String(node);
  if (node.type && node.type !== 'compound' && node.type !== 'list') return flatten(node.value);
  if (node.type === 'list') return (node.value?.value ?? node.value ?? []).map(flatten).join('|');
  return Object.values(node.value ?? node).map(flatten).filter(Boolean).join('|');
}

const client = mc.createClient({host: '127.0.0.1', port: 25565, username: 'TestBot', version: '1.21.4', auth: 'offline'});
let started = false;
const responses = [];
client.on('position', async packet => {
  client.write('teleport_confirm', {teleportId: packet.teleportId});
  if (started) return;
  started = true;
  await new Promise(r => setTimeout(r, 500));
  for (const command of commands) {
    client.write('chat_command', {command, timestamp: BigInt(Date.now())});
    await new Promise(r => setTimeout(r, mode !== 'query' && command.startsWith('save-all') ? 2500 : 600));
  }
  await new Promise(r => setTimeout(r, 500));
  client.end();
  console.log(JSON.stringify({mode, responses}));
});
client.on('system_chat', packet => responses.push(flatten(packet.content)));
client.on('error', error => { console.error(error); process.exit(1); });

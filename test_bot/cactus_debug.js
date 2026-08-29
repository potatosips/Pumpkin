const mc = require('minecraft-protocol');

function summarize(node) {
  if (node == null) return '';
  if (typeof node !== 'object') return String(node);
  if (node.type && node.type !== 'compound' && node.type !== 'list') return summarize(node.value);
  if (node.type === 'list') return (node.value?.value ?? node.value ?? []).map(summarize).join('');
  if (node.type === 'compound') return Object.values(node.value ?? {}).map(summarize).filter(Boolean).join('|');
  return Object.values(node).map(summarize).filter(Boolean).join('|');
}

const client = mc.createClient({ host: '127.0.0.1', port: 25575, username: 'TestBot', version: '1.21.4', auth: 'offline' });
client.on('position', () => {
  setTimeout(() => {
    client.write('chat_command', { command: 'tp @s 200 75 28', timestamp: BigInt(Date.now()) });
    client.write('chat_command', { command: 'fill 198 68 26 202 73 30 air', timestamp: BigInt(Date.now()) });
    setTimeout(() => {
      client.write('chat_command', { command: 'setblock 200 69 28 minecraft:sand', timestamp: BigInt(Date.now()) });
      client.write('chat_command', { command: 'setblock 200 70 28 minecraft:cactus', timestamp: BigInt(Date.now()) });
    }, 500);
    setTimeout(() => {
      client.write('chat_command', { command: 'execute if block 200 70 28 minecraft:cactus run say CACTUS_EXISTS', timestamp: BigInt(Date.now()) });
    }, 1000);
    setTimeout(() => client.end(), 2000);
  }, 500);
});
client.on('system_chat', packet => console.log('[CHAT]', summarize(packet.content)));

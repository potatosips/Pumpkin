const mc = require('minecraft-protocol');
const port = Number(process.env.PORT || 25565);
const label = process.env.LABEL || String(port);

const client = mc.createClient({host: '127.0.0.1', port, username: 'TestBot', version: '1.21.4', auth: 'offline'});
client.on('packet', (data, meta) => {
  if (meta.name !== 'declare_commands') return;
  const nodes = data.nodes;
  const nameOf = node => node?.name ?? node?.extraNodeData?.name;
  const index = nodes.findIndex(node => nameOf(node) === 'gamerule');
  if (index < 0) {
    console.log(JSON.stringify({label, error: 'gamerule node missing'}));
  } else {
    const node = nodes[index];
    const rules = (node.children || []).map(child => nameOf(nodes[child])).filter(Boolean).sort();
    console.log(JSON.stringify({label, node: index, count: rules.length, rules}, null, 2));
  }
  client.end();
});
client.on('error', error => { console.error(error.message); process.exitCode = 1; });

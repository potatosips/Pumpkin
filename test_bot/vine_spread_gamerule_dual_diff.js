const mc = require('minecraft-protocol');

const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
const fixtures = Array.from({length: 8}, (_, i) => ({x: 320 + i * 10, y: 90, z: 40}));

function flatten(node) {
  if (node == null) return '';
  if (typeof node !== 'object') return String(node);
  if (node.type && node.type !== 'compound' && node.type !== 'list') return flatten(node.value);
  if (node.type === 'list') return (node.value?.value ?? node.value ?? []).map(flatten).join('|');
  return Object.values(node.value ?? node).map(flatten).filter(Boolean).join('|');
}

function run(name, port) {
  return new Promise((resolve, reject) => {
    const client = mc.createClient({host: '127.0.0.1', port, username: 'TestBot', version: '1.21.4', auth: 'offline'});
    const messages = [];
    let started = false;
    const command = text => client.write('chat_command', {command: text, timestamp: BigInt(Date.now())});
    const send = async (text, wait = 180) => { command(text); await delay(wait); };
    const record = packet => messages.push(flatten(packet.message ?? packet.content ?? packet));
    for (const event of ['system_chat', 'profileless_chat', 'disguised_chat', 'player_chat']) client.on(event, record);

    client.on('position', async packet => {
      client.write('teleport_confirm', {teleportId: packet.teleportId});
      if (started) return;
      started = true;
      try {
        client.write('client_command', {actionId: 0});
        await delay(500);
        client.writeRaw(Buffer.from([0x2a]));
        await delay(700);
        await send('gamemode creative @s');
        await send('tp @s 355 92 40', 3000);
        await send('gamerule randomTickSpeed 1000');
        await send('gamerule doVinesSpread false');
        for (const f of fixtures) {
          await send(`fill ${f.x - 1} ${f.y - 2} ${f.z - 1} ${f.x + 1} ${f.y + 1} ${f.z + 1} air`, 60);
          await send(`setblock ${f.x} ${f.y} ${f.z - 1} stone`, 60);
          await send(`setblock ${f.x} ${f.y - 1} ${f.z - 1} stone`, 60);
          await send(`setblock ${f.x} ${f.y} ${f.z} vine[north=true]`, 60);
        }
        await delay(2500);
        for (let i = 0; i < fixtures.length; i++) {
          const f = fixtures[i];
          await send(`execute if block ${f.x} ${f.y} ${f.z} vine run say SOURCE_${i}`, 80);
          await send(`execute if block ${f.x} ${f.y - 1} ${f.z} vine run say FROZEN_BAD_${i}`, 80);
        }
        await send('gamerule doVinesSpread true');
        await delay(15000);
        for (let i = 0; i < fixtures.length; i++) {
          const f = fixtures[i];
          await send(`execute if block ${f.x} ${f.y - 1} ${f.z} vine run say ADVANCED_${i}`, 80);
        }
        await send('gamerule randomTickSpeed 3');
        await send('gamerule doVinesSpread true');
        const frozenBad = messages.filter(x => x.includes('FROZEN_BAD_')).length;
        const sources = messages.filter(x => x.includes('SOURCE_')).length;
        const advanced = messages.filter(x => x.includes('ADVANCED_')).length;
        const diagnostics = messages.filter(x => /unknown|incorrect|error/i.test(x));
        client.end();
        resolve({name, sources, frozenBad, advanced, diagnostics});
      } catch (error) { reject(error); }
    });
    client.on('error', reject);
  });
}

Promise.all([run('PUMPKIN', 25565), run('VANILLA', 25575)]).then(results => {
  for (const result of results) console.log(JSON.stringify(result));
  const pass = results.every(result => result.frozenBad === 0 && result.advanced > 0 && result.diagnostics.length === 0);
  console.log(`VINE_SPREAD_GAMERULE=${pass ? 'PASS' : 'FAIL'}`);
  if (!pass) process.exitCode = 1;
}).catch(error => { console.error(error); process.exit(1); });

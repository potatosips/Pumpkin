const mc = require('minecraft-protocol');

const delay = milliseconds => new Promise(resolve => setTimeout(resolve, milliseconds));

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
    const markers = new Set();
    const diagnostics = [];
    let explosionPackets = 0;
    let started = false;
    const command = text => client.write('chat_command', {command: text, timestamp: BigInt(Date.now())});
    const send = async (text, wait = 300) => { command(text); await delay(wait); };

    const record = packet => {
      const text = flatten(packet.message ?? packet.content ?? packet);
      for (const marker of text.match(/(?:CONTROL_REMAINS|IGNITED_EXPLODED)/g) ?? []) markers.add(marker);
      if (/error|unknown|incorrect|failed/i.test(text)) diagnostics.push(text);
    };
    client.on('system_chat', record);
    client.on('profileless_chat', record);
    client.on('disguised_chat', record);
    client.on('player_chat', record);
    client.on('explosion', () => { explosionPackets++; });

    client.on('position', async packet => {
      client.write('teleport_confirm', {teleportId: packet.teleportId});
      if (started) return;
      started = true;
      try {
        client.write('client_command', {actionId: 0});
        await delay(600);
        client.writeRaw(Buffer.from([0x2a]));
        await delay(800);
        await send('gamemode spectator @s');
        await send('tp @s 738 85 0', 3000);
        await send('gamerule mobGriefing false');
        await send('kill @e[type=creeper,x=725,y=60,z=-8,dx=20,dy=40,dz=16]');

        await send('summon creeper 730.5 80 0.5 {NoAI:1b,NoGravity:1b,Fuse:20s,ExplosionRadius:3b}');
        await delay(2200);
        await send('execute if entity @e[type=creeper,x=729,y=79,z=-1,dx=3,dy=3,dz=3,limit=1] run say CONTROL_REMAINS');
        await send('kill @e[type=creeper,x=725,y=60,z=-8,dx=20,dy=40,dz=16]');

        explosionPackets = 0;
        await send('summon creeper 730.5 80 0.5 {NoAI:1b,NoGravity:1b,ignited:1b,Fuse:20s,ExplosionRadius:3b}');
        await delay(2500);
        await send('execute unless entity @e[type=creeper,x=729,y=79,z=-1,dx=3,dy=3,dz=3,limit=1] run say IGNITED_EXPLODED');
        await send('gamerule mobGriefing true');
        await send('kill @e[type=creeper,x=725,y=60,z=-8,dx=20,dy=40,dz=16]');
        client.end();
        resolve({name, markers: [...markers].sort(), explosionPackets, diagnostics: diagnostics.slice(0, 10)});
      } catch (error) {
        reject(error);
      }
    });
    client.on('error', reject);
  });
}

Promise.all([run('PUMPKIN', 25565), run('VANILLA', 25575)]).then(results => {
  for (const result of results) console.log(JSON.stringify(result));
  const valid = results.every(result =>
    result.markers.includes('CONTROL_REMAINS')
    && result.markers.includes('IGNITED_EXPLODED')
    && result.explosionPackets >= 1
    && result.diagnostics.length === 0
  );
  console.log(`NOAI_CREEPER_FUSE_BEHAVIOR=${valid ? 'PASS' : 'FAIL'}`);
  if (!valid) process.exitCode = 1;
}).catch(error => {
  console.error(error);
  process.exit(1);
});

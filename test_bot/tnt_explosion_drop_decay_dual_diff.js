const mc = require('minecraft-protocol');

const delay = milliseconds => new Promise(resolve => setTimeout(resolve, milliseconds));
const TRIALS = 12;

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
    const messages = [];
    let started = false;
    const command = text => client.write('chat_command', {command: text, timestamp: BigInt(Date.now())});
    const send = async (text, wait = 250) => { command(text); await delay(wait); };

    const record = packet => {
      const text = flatten(packet.message ?? packet.content ?? packet);
      messages.push(text);
      const match = text.match(/(?:DROP|DESTROYED)_(FALSE|TRUE)_\d+/);
      if (match) markers.add(match[0]);
    };
    client.on('system_chat', record);
    client.on('profileless_chat', record);
    client.on('disguised_chat', record);
    client.on('player_chat', record);
    client.on('position', async packet => {
      client.write('teleport_confirm', {teleportId: packet.teleportId});
      if (started) return;
      started = true;
      try {
        client.write('client_command', {actionId: 0});
        await delay(600);
        client.writeRaw(Buffer.from([0x2a]));
        await delay(800);
        await send('gamemode creative @s');
        await send('tp @s 708 80 0', 4000);
        await send('gamerule doTileDrops true');

        for (const [label, value] of [['FALSE', 'false'], ['TRUE', 'true']]) {
          await send(`gamerule tntExplosionDropDecay ${value}`);
          for (let trial = 0; trial < TRIALS; trial++) {
            await send('kill @e[type=item,x=694,y=0,z=-6,dx=12,dy=100,dz=12]', 150);
            await send('kill @e[type=tnt,x=694,y=0,z=-6,dx=12,dy=100,dz=12]', 150);
            await send('fill 697 68 -3 703 73 3 air', 150);
            await send('setblock 700 69 0 stone', 150);
            await send('setblock 700 70 0 tnt', 150);
            await send('setblock 701 70 0 redstone_block', 5000);
            await send(`execute if block 700 69 0 air run say DESTROYED_${label}_${trial}`, 150);
            await send(`execute if entity @e[type=item,x=697,y=0,z=-3,dx=6,dy=100,dz=6] run say DROP_${label}_${trial}`, 200);
          }
        }

        await send('gamerule tntExplosionDropDecay false');
        await send('gamerule doTileDrops true');
        await send('kill @e[type=item,x=694,y=0,z=-6,dx=12,dy=100,dz=12]');
        await send('fill 697 68 -3 703 73 3 air');
        await send('gamemode creative @s');
        client.end();
        resolve({
          name,
          noDecayDrops: [...markers].filter(marker => marker.startsWith('DROP_FALSE_')).length,
          decayDrops: [...markers].filter(marker => marker.startsWith('DROP_TRUE_')).length,
          noDecayDestroyed: [...markers].filter(marker => marker.startsWith('DESTROYED_FALSE_')).length,
          decayDestroyed: [...markers].filter(marker => marker.startsWith('DESTROYED_TRUE_')).length,
          markers: [...markers].sort()
          , messages: messages.filter(message => message.includes('error') || message.includes('argument') || message.includes('summon')).slice(0, 12)
        });
      } catch (error) { reject(error); }
    });
    client.on('error', reject);
  });
}

Promise.all([run('PUMPKIN', 25565), run('VANILLA', 25575)]).then(results => {
  for (const result of results) console.log(JSON.stringify(result));
  const valid = results.every(result =>
    result.noDecayDrops >= 10
    && result.decayDrops < result.noDecayDrops
    && result.decayDrops <= 8);
  console.log(`TNT_EXPLOSION_DROP_DECAY_BEHAVIOR=${valid ? 'PASS' : 'FAIL'}`);
  if (!valid) process.exitCode = 1;
}).catch(error => {
  console.error(error);
  process.exit(1);
});

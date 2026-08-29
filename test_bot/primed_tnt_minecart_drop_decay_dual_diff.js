const mc = require('minecraft-protocol');

const delay = milliseconds => new Promise(resolve => setTimeout(resolve, milliseconds));
const TRIALS = 10;

function flatten(node) {
  if (node == null) return '';
  if (typeof node !== 'object') return String(node);
  if (node.type && node.type !== 'compound' && node.type !== 'list') return flatten(node.value);
  if (node.type === 'list') return (node.value?.value ?? node.value ?? []).map(flatten).join('|');
  return Object.values(node.value ?? node).map(flatten).filter(Boolean).join('|');
}

function count(markers, prefix) {
  return [...markers].filter(marker => marker.startsWith(prefix)).length;
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
      for (const marker of text.match(/MINECART_(?:IMPACT|DESTROYED|DROP|RAIL|SUPPORT)_(?:FALSE|TRUE)_\d+/g) ?? []) {
        markers.add(marker);
      }
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
        await send('tp @s 712 90 0', 3500);
        await send('gamerule doTileDrops true');

        for (const [label, value] of [['FALSE', 'false'], ['TRUE', 'true']]) {
          await send(`gamerule tntExplosionDropDecay ${value}`);
          for (let trial = 0; trial < TRIALS; trial++) {
            await send('kill @e[type=tnt_minecart,x=690,y=50,z=-10,dx=25,dy=50,dz=20]');
            await send('kill @e[type=item,x=690,y=40,z=-10,dx=25,dy=70,dz=20]');
            await send('fill 690 50 -10 710 95 10 air');
            await send('setblock 700 79 0 stone');
            await send('setblock 700 80 0 rail');
            await send('setblock 702 80 0 stone');
            await send('summon tnt_minecart 700.5 80 0.5 {fuse:1,NoGravity:1b}', 2500);
            await send(`execute unless entity @e[type=tnt_minecart,x=697,y=75,z=-3,dx=8,dy=10,dz=6] run say MINECART_IMPACT_${label}_${trial}`);
            await send(`execute unless block 702 80 0 stone run say MINECART_DESTROYED_${label}_${trial}`);
            await send(`execute if entity @e[type=item,x=690,y=40,z=-10,dx=20,dy=70,dz=20] run say MINECART_DROP_${label}_${trial}`);
            await send(`execute if block 700 80 0 rail run say MINECART_RAIL_${label}_${trial}`);
            await send(`execute if block 700 79 0 stone run say MINECART_SUPPORT_${label}_${trial}`);
          }
        }

        await send('gamerule tntExplosionDropDecay false');
        await send('gamerule doTileDrops true');
        await send('kill @e[type=tnt_minecart,x=690,y=50,z=-10,dx=25,dy=50,dz=20]');
        await send('kill @e[type=item,x=690,y=40,z=-10,dx=25,dy=70,dz=20]');
        await send('fill 690 50 -10 710 95 10 air');
        client.end();

        const result = {name, explosionPackets, diagnostics: diagnostics.slice(0, 15)};
        for (const state of ['FALSE', 'TRUE']) {
          for (const kind of ['IMPACT', 'DESTROYED', 'DROP', 'RAIL', 'SUPPORT']) {
            result[`${state}_${kind}`] = count(markers, `MINECART_${kind}_${state}_`);
          }
        }
        result.markers = [...markers].sort();
        resolve(result);
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
    result.explosionPackets === TRIALS * 2
    && result.FALSE_IMPACT === TRIALS
    && result.TRUE_IMPACT === TRIALS
    && result.FALSE_DESTROYED === TRIALS
    && result.TRUE_DESTROYED === TRIALS
    && result.FALSE_DROP === TRIALS
    && result.TRUE_DROP < result.FALSE_DROP
    && result.TRUE_DROP <= 7
    && result.FALSE_RAIL === TRIALS
    && result.TRUE_RAIL === TRIALS
    && result.FALSE_SUPPORT === TRIALS
    && result.TRUE_SUPPORT === TRIALS
  );
  console.log(`PRIMED_TNT_MINECART_DROP_DECAY_BEHAVIOR=${valid ? 'PASS' : 'FAIL'}`);
  if (!valid) process.exitCode = 1;
}).catch(error => {
  console.error(error);
  process.exit(1);
});

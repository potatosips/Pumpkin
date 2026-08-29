const mc = require('minecraft-protocol');

const delay = milliseconds => new Promise(resolve => setTimeout(resolve, milliseconds));
const DECAY_TRIALS = 10;
const GRIEF_TRIALS = 6;

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
      for (const marker of text.match(/FIREBALL_(?:IMPACT|REMAINED|DESTROYED|DROP|FIRE)_(?:GRIEF_FALSE|DECAY_FALSE|DECAY_TRUE)_\d+/g) ?? []) {
        markers.add(marker);
      }
      if (/error|unknown|incorrect|failed/i.test(text)) diagnostics.push(text);
    };
    client.on('system_chat', record);
    client.on('profileless_chat', record);
    client.on('disguised_chat', record);
    client.on('player_chat', record);
    client.on('explosion', () => { explosionPackets++; });

    const prepare = async () => {
      await send('kill @e[type=fireball,x=690,y=50,z=-10,dx=25,dy=50,dz=20]');
      await send('kill @e[type=item,x=690,y=40,z=-10,dx=25,dy=70,dz=20]');
      await send('fill 690 50 -10 710 95 10 air');
      await send('setblock 700 80 0 stone');
    };

    const launch = async () => {
      await send('summon fireball 704 80.5 0.5 {Motion:[-0.5d,0.0d,0.0d],acceleration_power:0.1d,ExplosionPower:4b}', 2500);
    };

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
        await send('gamerule mobExplosionDropDecay false');
        await send('gamerule mobGriefing false');

        for (let trial = 0; trial < GRIEF_TRIALS; trial++) {
          await prepare();
          await launch();
          await send(`execute unless entity @e[type=fireball,x=698,y=75,z=-3,dx=12,dy=12,dz=6] run say FIREBALL_IMPACT_GRIEF_FALSE_${trial}`);
          await send(`execute if block 700 80 0 stone run say FIREBALL_REMAINED_GRIEF_FALSE_${trial}`);
          await send(`execute if block 700 81 0 fire run say FIREBALL_FIRE_GRIEF_FALSE_${trial}`);
        }

        await send('gamerule mobGriefing true');
        for (const [label, value] of [['DECAY_FALSE', 'false'], ['DECAY_TRUE', 'true']]) {
          await send(`gamerule mobExplosionDropDecay ${value}`);
          for (let trial = 0; trial < DECAY_TRIALS; trial++) {
            await prepare();
            await launch();
            await send(`execute unless entity @e[type=fireball,x=698,y=75,z=-3,dx=12,dy=12,dz=6] run say FIREBALL_IMPACT_${label}_${trial}`);
            await send(`execute unless block 700 80 0 stone run say FIREBALL_DESTROYED_${label}_${trial}`);
            await send(`execute if entity @e[type=item,x=690,y=40,z=-10,dx=20,dy=70,dz=20] run say FIREBALL_DROP_${label}_${trial}`);
          }
        }

        await send('gamerule mobExplosionDropDecay true');
        await send('gamerule mobGriefing true');
        await send('gamerule doTileDrops true');
        await prepare();
        client.end();

        resolve({
          name,
          explosionPackets,
          griefImpacts: count(markers, 'FIREBALL_IMPACT_GRIEF_FALSE_'),
          griefRemained: count(markers, 'FIREBALL_REMAINED_GRIEF_FALSE_'),
          griefFire: count(markers, 'FIREBALL_FIRE_GRIEF_FALSE_'),
          noDecayImpacts: count(markers, 'FIREBALL_IMPACT_DECAY_FALSE_'),
          noDecayDestroyed: count(markers, 'FIREBALL_DESTROYED_DECAY_FALSE_'),
          noDecayDrops: count(markers, 'FIREBALL_DROP_DECAY_FALSE_'),
          decayImpacts: count(markers, 'FIREBALL_IMPACT_DECAY_TRUE_'),
          decayDestroyed: count(markers, 'FIREBALL_DESTROYED_DECAY_TRUE_'),
          decayDrops: count(markers, 'FIREBALL_DROP_DECAY_TRUE_'),
          diagnostics: diagnostics.slice(0, 15),
          markers: [...markers].sort()
        });
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
    result.griefImpacts === GRIEF_TRIALS
    && result.griefRemained === GRIEF_TRIALS
    && result.noDecayImpacts === DECAY_TRIALS
    && result.noDecayDestroyed === DECAY_TRIALS
    && result.noDecayDrops === DECAY_TRIALS
    && result.decayImpacts === DECAY_TRIALS
    && result.decayDestroyed === DECAY_TRIALS
    && result.decayDrops < result.noDecayDrops
    && result.decayDrops <= 7
  );
  console.log(`LARGE_FIREBALL_GAMERULE_BEHAVIOR=${valid ? 'PASS' : 'FAIL'}`);
  if (!valid) process.exitCode = 1;
}).catch(error => {
  console.error(error);
  process.exit(1);
});

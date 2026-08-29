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

function summarize(markers, source, state, kind) {
  const prefix = `${source}_${kind}_${state}_`;
  return [...markers].filter(marker => marker.startsWith(prefix)).length;
}

function run(name, port) {
  return new Promise((resolve, reject) => {
    // TestBot is the established operator on the Vanilla fixture. A fresh
    // username is subject to Vanilla's command-spam disconnect accounting.
    const client = mc.createClient({host: '127.0.0.1', port, username: 'TestBot', version: '1.21.4', auth: 'offline'});
    const markers = new Set();
    const diagnostics = [];
    let started = false;
    const command = text => client.write('chat_command', {command: text, timestamp: BigInt(Date.now())});
    // Vanilla disconnects command clients that sustain the 100 ms setup rate
    // used by the first calibration. Keep every command at least 400 ms apart.
    const send = async (text, wait = 400) => { command(text); await delay(Math.max(wait, 400)); };

    const record = packet => {
      const text = flatten(packet.message ?? packet.content ?? packet);
      const matches = text.match(/(?:MOB|BLOCK)_(?:DROP|DESTROYED)_(?:FALSE|TRUE)_\d+/g) ?? [];
      for (const marker of matches) markers.add(marker);
      if (/error|argument|unknown|incorrect|failed|no entity/i.test(text)) diagnostics.push(text);
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
        await send('gamemode spectator @s');
        await send('tp @s 708 90 0', 4000);
        await send('gamerule doTileDrops true');
        await send('gamerule mobGriefing true');
        // Isolate the single loot-bearing block from natural terrain so an
        // unrelated dirt/stone drop cannot satisfy the item-entity assertion.
        await send('fill 690 50 -10 710 95 10 air', 1200);

        for (const [label, value] of [['FALSE', 'false'], ['TRUE', 'true']]) {
          await send(`gamerule mobExplosionDropDecay ${value}`);
          for (let trial = 0; trial < TRIALS; trial++) {
            await send('kill @e[type=item,x=690,y=40,z=-10,dx=20,dy=70,dz=20]');
            await send('kill @e[type=creeper,x=690,y=40,z=-10,dx=20,dy=70,dz=20]');
            await send('setblock 700 80 0 stone');
            // Pumpkin currently suppresses mob_tick entirely for NoAI mobs,
            // including the creeper fuse. NoGravity pins the one-tick fuse
            // without invoking that separate lifecycle discrepancy.
            await send('summon creeper 700.5 81 0.5 {ignited:1b,Fuse:1s,ExplosionRadius:4b,NoGravity:1b}', 3000);
            await send(`execute unless block 700 80 0 stone run say MOB_DESTROYED_${label}_${trial}`);
            await send(`execute if entity @e[type=item,x=690,y=40,z=-10,dx=20,dy=70,dz=20] run say MOB_DROP_${label}_${trial}`);
          }
        }

        for (const [label, value] of [['FALSE', 'false'], ['TRUE', 'true']]) {
          await send(`gamerule blockExplosionDropDecay ${value}`);
          for (let trial = 0; trial < TRIALS; trial++) {
            await send('kill @e[type=item,x=690,y=40,z=-10,dx=20,dy=70,dz=20]');
            await send('kill @e[type=end_crystal,x=690,y=40,z=-10,dx=20,dy=70,dz=20]');
            await send('setblock 700 80 0 stone');
            await send('summon end_crystal 700.5 81 0.5 {ShowBottom:0b}');
            await send('damage @e[type=end_crystal,x=699,y=80,z=-1,dx=3,dy=3,dz=3,limit=1] 1 minecraft:generic', 3000);
            await send(`execute unless block 700 80 0 stone run say BLOCK_DESTROYED_${label}_${trial}`);
            await send(`execute if entity @e[type=item,x=690,y=40,z=-10,dx=20,dy=70,dz=20] run say BLOCK_DROP_${label}_${trial}`);
          }
        }

        await send('gamerule mobExplosionDropDecay true');
        await send('gamerule blockExplosionDropDecay true');
        await send('gamerule mobGriefing true');
        await send('gamerule doTileDrops true');
        await send('kill @e[type=item,x=690,y=40,z=-10,dx=20,dy=70,dz=20]');
        await send('kill @e[type=creeper,x=690,y=40,z=-10,dx=20,dy=70,dz=20]');
        await send('kill @e[type=end_crystal,x=690,y=40,z=-10,dx=20,dy=70,dz=20]');
        await send('fill 690 50 -10 710 95 10 air');
        client.end();

        const result = {name, diagnostics: diagnostics.slice(0, 20), markers: [...markers].sort()};
        for (const source of ['MOB', 'BLOCK']) {
          result[`${source}_FALSE_destroyed`] = summarize(markers, source, 'FALSE', 'DESTROYED');
          result[`${source}_FALSE_drops`] = summarize(markers, source, 'FALSE', 'DROP');
          result[`${source}_TRUE_destroyed`] = summarize(markers, source, 'TRUE', 'DESTROYED');
          result[`${source}_TRUE_drops`] = summarize(markers, source, 'TRUE', 'DROP');
        }
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
  const valid = results.every(result => ['MOB', 'BLOCK'].every(source =>
    result[`${source}_FALSE_destroyed`] === TRIALS
    && result[`${source}_TRUE_destroyed`] === TRIALS
    && result[`${source}_FALSE_drops`] >= 10
    && result[`${source}_TRUE_drops`] < result[`${source}_FALSE_drops`]
    && result[`${source}_TRUE_drops`] <= 8
  ));
  console.log(`MOB_BLOCK_EXPLOSION_DROP_DECAY_BEHAVIOR=${valid ? 'PASS' : 'FAIL'}`);
  if (!valid) process.exitCode = 1;
}).catch(error => {
  console.error(error);
  process.exit(1);
});

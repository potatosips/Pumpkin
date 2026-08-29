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
    const fires = new Set();
    const checks = new Set();
    const diagnostics = [];
    let pendingCheck = null;
    let explosionPackets = 0;
    let started = false;
    const command = text => client.write('chat_command', {command: text, timestamp: BigInt(Date.now())});
    const send = async (text, wait = 300) => { command(text); await delay(wait); };

    const record = packet => {
      const text = flatten(packet.message ?? packet.content ?? packet);
      const match = text.match(/FIRE_CHECK_(FALSE|TRUE)_\d+/);
      if (match) {
        pendingCheck = match[0];
        checks.add(match[0]);
        return;
      }
      if (pendingCheck && text.includes('commands.fill.success')) {
        fires.add(pendingCheck);
        pendingCheck = null;
        return;
      }
      if (pendingCheck && text.includes('commands.fill.failed')) {
        pendingCheck = null;
        return;
      }
      if (/error|unknown|incorrect/i.test(text)) diagnostics.push(text);
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

        for (const [label, value] of [['FALSE', 'false'], ['TRUE', 'true']]) {
          await send(`gamerule mobGriefing ${value}`);
          for (let trial = 0; trial < TRIALS; trial++) {
            await send('kill @e[type=fireball,x=690,y=50,z=-10,dx=25,dy=50,dz=20]');
            await send('fill 696 80 -4 704 84 4 air');
            await send('fill 696 79 -4 704 79 4 bedrock');
            await send('setblock 700 80 0 bedrock');
            await send('summon fireball 704 80.5 0.5 {Motion:[-0.5d,0.0d,0.0d],acceleration_power:0.1d,ExplosionPower:1b}', 2200);
            await send(`say FIRE_CHECK_${label}_${trial}`);
            await send('fill 696 80 -4 704 84 4 air replace fire', 500);
          }
        }

        await send('gamerule mobGriefing true');
        await send('kill @e[type=fireball,x=690,y=50,z=-10,dx=25,dy=50,dz=20]');
        await send('fill 696 79 -4 704 84 4 air');
        client.end();
        resolve({
          name,
          explosionPackets,
          checks: checks.size,
          falseFireTrials: [...fires].filter(marker => marker.startsWith('FIRE_CHECK_FALSE_')).length,
          trueFireTrials: [...fires].filter(marker => marker.startsWith('FIRE_CHECK_TRUE_')).length,
          diagnostics,
          fires: [...fires].sort()
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
    result.explosionPackets === TRIALS * 2
    && result.checks === TRIALS * 2
    && result.falseFireTrials === 0
    && result.trueFireTrials > 0
    && result.diagnostics.length === 0
  );
  console.log(`LARGE_FIREBALL_FIRE_BEHAVIOR=${valid ? 'PASS' : 'FAIL'}`);
  if (!valid) process.exitCode = 1;
}).catch(error => {
  console.error(error);
  process.exit(1);
});

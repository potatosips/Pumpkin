const mc = require('minecraft-protocol');

const delay = ms => new Promise(resolve => setTimeout(resolve, ms));

function connect(username, port) {
  return new Promise((resolve, reject) => {
    const client = mc.createClient({host: '127.0.0.1', port, username, version: '1.21.4', auth: 'offline'});
    let joined = false;
    client.on('position', packet => {
      client.write('teleport_confirm', {teleportId: packet.teleportId});
      if (!joined) {
        joined = true;
        client.write('client_command', {actionId: 0});
        setTimeout(() => client.writeRaw(Buffer.from([0x2a])), 150);
        resolve(client);
      }
    });
    client.on('error', reject);
  });
}

async function run(name, port) {
  const suffix = `${port}${Date.now() % 10000}`;
  const attackerName = `FFA${suffix}`.slice(0, 16);
  const victimName = `FFV${suffix}`.slice(0, 16);
  const admin = await connect('TestBot', port);
  const attacker = await connect(attackerName, port);
  const victim = await connect(victimName, port);
  let health = 20;
  const healthHistory = [];
  victim.on('update_health', packet => {
    health = packet.health;
    healthHistory.push(packet.health);
  });

  const command = async (text, wait = 300) => {
    admin.write('chat_command', {command: text, timestamp: BigInt(Date.now())});
    await delay(wait);
  };
  const heal = async () => {
    await command(`effect give ${victimName} minecraft:instant_health 1 10 true`, 450);
  };
  const observe = async (label, commandText, expected) => {
    healthHistory.length = 0;
    const before = health;
    await command(commandText, 550);
    const minimum = Math.min(before, ...healthHistory);
    return {label, before, after: health, minimum, expected, healthHistory: [...healthHistory]};
  };

  await command(`gamemode survival ${attackerName}`);
  await command(`gamemode survival ${victimName}`);
  await command('gamerule naturalRegeneration false');
  await command('team remove ffsafe');
  await command('team remove ffenemy');
  await command('team add ffsafe');
  await command('team add ffenemy');
  await command(`team join ffsafe ${attackerName}`);
  await command(`team join ffsafe ${victimName}`);
  await command('team modify ffsafe friendlyFire false');
  await heal();

  const observations = [];
  observations.push(await observe(
    'same_team_direct_disabled',
    `damage ${victimName} 4 minecraft:player_attack by ${attackerName}`,
    20,
  ));

  await command('team modify ffsafe friendlyFire true');
  observations.push(await observe(
    'same_team_direct_enabled',
    `damage ${victimName} 4 minecraft:player_attack by ${attackerName}`,
    16,
  ));
  await heal();

  await command('team modify ffsafe friendlyFire false');
  observations.push(await observe(
    'same_team_arrow_disabled',
    `damage ${victimName} 4 minecraft:arrow by ${attackerName} from ${attackerName}`,
    20,
  ));

  await command(`team join ffenemy ${attackerName}`);
  observations.push(await observe(
    'different_team',
    `damage ${victimName} 4 minecraft:player_attack by ${attackerName}`,
    16,
  ));

  await command('team remove ffsafe');
  await command('team remove ffenemy');
  await command('gamerule naturalRegeneration true');
  for (const client of [admin, attacker, victim]) client.end();
  const pass = observations.every(result => Math.abs(result.minimum - result.expected) < 0.01);
  return {name, pass, observations};
}

Promise.all([run('PUMPKIN', 25565), run('VANILLA', 25575)]).then(results => {
  for (const result of results) console.log(JSON.stringify(result));
  const pass = results.every(result => result.pass);
  console.log(`FRIENDLY_FIRE=${pass ? 'PASS' : 'FAIL'}`);
  if (!pass) process.exitCode = 1;
}).catch(error => { console.error(error); process.exit(1); });

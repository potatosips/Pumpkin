const mc = require('minecraft-protocol');

const delay = ms => new Promise(resolve => setTimeout(resolve, ms));

function connect(username, port) {
  return new Promise((resolve, reject) => {
    const client = mc.createClient({host: '127.0.0.1', port, username, version: '1.21.4', auth: 'offline'});
    let ready = false;
    client.on('position', packet => {
      client.write('teleport_confirm', {teleportId: packet.teleportId});
      if (!ready) {
        ready = true;
        client.write('client_command', {actionId: 0});
        setTimeout(() => client.writeRaw(Buffer.from([0x2a])), 150);
        resolve(client);
      }
    });
    client.on('error', reject);
  });
}

async function run(name, port, x) {
  const admin = await connect('TestBot', port);
  const sleeper = await connect('SleepBot', port);
  const spectator = await connect('SpecBot', port);
  const times = [];
  admin.on('update_time', packet => times.push(Number(packet.time)));
  const command = async (text, wait = 220) => {
    admin.write('chat_command', {command: text, timestamp: BigInt(Date.now())});
    await delay(wait);
  };

  await command('gamerule playersSleepingPercentage 100');
  await command(`fill ${x - 2} 78 -3 ${x + 2} 82 3 air`);
  await command(`fill ${x - 2} 79 -3 ${x + 2} 79 3 stone`);
  await command(`setblock ${x} 80 0 red_bed[part=foot,facing=south,occupied=false]`);
  await command(`setblock ${x} 80 1 red_bed[part=head,facing=south,occupied=false]`);
  await command(`tp SleepBot ${x + 0.5} 80 -0.5 0 20`);
  await command(`tp TestBot ${x + 3.5} 82 0`);
  await command(`tp SpecBot ${x + 4.5} 82 0`);
  await command('gamemode spectator TestBot');
  await command('gamemode spectator SpecBot');
  await command('time set night', 600);
  const before = times.length;

  sleeper.write('block_place', {
    hand: 0,
    location: {x, y: 80, z: 0},
    direction: 1,
    cursorX: 0.5,
    cursorY: 0.5,
    cursorZ: 0.5,
    insideBlock: false,
    worldBorderHit: false,
    sequence: 1
  });
  await delay(7500);

  const observed = times.slice(before);
  const skipped = observed.some(time => {
    const day = ((time % 24000) + 24000) % 24000;
    return day < 1000;
  });
  await command('gamemode creative TestBot');
  await command('time set day');
  for (const client of [admin, sleeper, spectator]) client.end();
  return {name, skipped, samples: observed.slice(-8)};
}

Promise.all([
  run('PUMPKIN', 25565, 500),
  run('VANILLA', 25575, 520)
]).then(results => {
  for (const result of results) console.log(JSON.stringify(result));
  const pass = results.every(result => result.skipped);
  console.log(`SLEEP_PERCENTAGE_SPECTATOR=${pass ? 'PASS' : 'FAIL'}`);
  if (!pass) process.exitCode = 1;
}).catch(error => { console.error(error); process.exit(1); });

const mc = require('minecraft-protocol');

const delay = ms => new Promise(resolve => setTimeout(resolve, ms));

function connect(username, port) {
  return new Promise((resolve, reject) => {
    const client = mc.createClient({host: '127.0.0.1', port, username, version: '1.21.4', auth: 'offline'});
    let first = true;
    client.on('position', packet => {
      client.write('teleport_confirm', {teleportId: packet.teleportId});
      if (first) {
        first = false;
        client.write('client_command', {actionId: 0});
        resolve({client, firstPosition: {x: packet.x, y: packet.y, z: packet.z}});
      }
    });
    client.on('error', reject);
  });
}

async function run(name, port, x) {
  const targetName = `SR${port}${Date.now() % 1000000}`;
  const {client: admin} = await connect('TestBot', port);
  const command = async (text, wait = 220) => {
    admin.write('chat_command', {command: text, timestamp: BigInt(Date.now())});
    await delay(wait);
  };
  await command('gamerule spawnRadius 0');
  await command(`fill ${x - 3} 78 -3 ${x + 3} 84 3 air`);
  await command(`fill ${x - 3} 79 -3 ${x + 3} 79 3 stone`);
  await command(`setworldspawn ${x} 80 0 0`);
  const {client: target, firstPosition} = await connect(targetName, port);
  const exact = Math.abs(firstPosition.x - (x + 0.5)) < 0.01
    && Math.abs(firstPosition.z - 0.5) < 0.01;
  await command('gamerule spawnRadius 10');
  for (const client of [admin, target]) client.end();
  return {name, exact, targetName, firstPosition};
}

Promise.all([run('PUMPKIN', 25565, 600), run('VANILLA', 25575, 620)]).then(results => {
  for (const result of results) console.log(JSON.stringify(result));
  const pass = results.every(result => result.exact);
  console.log(`SPAWN_RADIUS_ZERO=${pass ? 'PASS' : 'FAIL'}`);
  if (!pass) process.exitCode = 1;
}).catch(error => { console.error(error); process.exit(1); });

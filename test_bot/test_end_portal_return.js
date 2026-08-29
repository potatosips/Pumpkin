const mc = require('minecraft-protocol');
const probeUsername = `endprobe${String(Date.now()).slice(-8)}`;

const client = mc.createClient({
  host: '127.0.0.1',
  port: 25565,
  username: probeUsername,
  version: '1.21.4',
  auth: 'offline'
});

let dimension = null;
let sawEnd = false;
let sawReturn = false;
let ended = false;
let entryScheduled = false;
let exitScheduled = false;
let returnPosition = null;

function command(value) {
  client.write('chat_command', { command: value, timestamp: BigInt(Date.now()) });
}

function finish(code, message) {
  if (ended) return;
  ended = true;
  console.log(message);
  client.end();
  setTimeout(() => process.exit(code), 100);
}

client.on('packet', (packet, meta) => {
  if (meta.name !== 'login' && meta.name !== 'respawn') return;
  const spawnInfo = packet.worldState || packet.commonSpawnInfo || packet;
  dimension = String(spawnInfo.name || spawnInfo.dimensionName || '');
  if (dimension.includes('the_end')) sawEnd = true;
  if (sawEnd && dimension.includes('overworld')) sawReturn = true;
  console.log(`[EndPortalProbe] ${meta.name} dimension=${dimension}`);
  if (meta.name === 'respawn' && dimension.includes('the_end') && !exitScheduled) {
    exitScheduled = true;
    setTimeout(() => command('setblock 100 49 0 minecraft:end_portal'), 2000);
    setTimeout(() => {
      if (dimension.includes('the_end')) command('teleport 100.5 49 0.5');
    }, 2500);
  }
});

client.on('position', (packet) => {
  if (sawReturn && dimension.includes('overworld')) {
    returnPosition = { x: packet.x, y: packet.y, z: packet.z };
  }
  if (packet.teleportId !== undefined) {
    client.write('teleport_confirm', { teleportId: packet.teleportId });
  }
});

client.on('system_chat', (packet) => {
  if (!entryScheduled && JSON.stringify(packet.content).includes('commands.spawnpoint.success.single')) {
    entryScheduled = true;
    setTimeout(() => command('setblock 0 81 0 minecraft:end_portal'), 500);
  }
});

client.on('login', () => {
  console.log('[EndPortalProbe] Logged in');
  setTimeout(() => command('setblock 0 81 0 minecraft:air'), 300);
  setTimeout(() => command('teleport 0 81 0'), 1500);
  setTimeout(() => command('setblock 0 80 0 minecraft:stone'), 2200);
  setTimeout(() => command('spawnpoint @s 0 81 0'), 3000);
  setTimeout(() => {
    const returnedToStoredSpawn = returnPosition
      && Math.abs(returnPosition.x - 0.5) < 0.01
      && Math.abs(returnPosition.y - 81.1) < 0.01
      && Math.abs(returnPosition.z - 0.5) < 0.01;
    finish(
      sawEnd && sawReturn && returnedToStoredSpawn ? 0 : 1,
      sawEnd && sawReturn && returnedToStoredSpawn
        ? '[PASS] End exit portal returned player to stored Overworld spawn'
        : `[FAIL] sawEnd=${sawEnd} sawReturn=${sawReturn} lastDimension=${dimension} returnPosition=${JSON.stringify(returnPosition)}`
    );
  }, 25000);
});

client.on('error', error => finish(1, `[FAIL] Protocol error: ${error.stack || error}`));
client.on('end', reason => {
  if (!ended) finish(1, `[FAIL] Disconnected early: ${reason}`);
});

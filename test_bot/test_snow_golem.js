const mc = require('minecraft-protocol');

const client = mc.createClient({
  host: '127.0.0.1',
  port: 25565,
  username: 'TestBot',
  version: '1.21.4',
  auth: 'offline'
});

console.log('Connecting to Pumpkin server at 127.0.0.1:25565...');

let entityCount = 0;
let snowballsSpawned = 0;
let entityPositions = {};

client.on('connect', () => {
  console.log('[TestBot] TCP Connected.');
});

client.on('login', (packet) => {
  console.log('[TestBot] Login successful! Game mode:', packet.gameMode, 'EntityId:', packet.entityId);
  
  setTimeout(() => {
    console.log('[TestBot] Summoning Snow Golem and Zombie for live AI test...');
    client.write('chat_command', { command: 'gamemode creative', timestamp: BigInt(Date.now()) });
    client.write('chat_command', { command: 'summon snow_golem ~ ~ ~', timestamp: BigInt(Date.now()) });
    client.write('chat_command', { command: 'summon zombie ~5 ~ ~', timestamp: BigInt(Date.now()) });
  }, 1000);

  setTimeout(() => {
    console.log(`[TestBot] Test summary:`);
    console.log(`- Entities tracked: ${Object.keys(entityPositions).length}`);
    console.log(`- Snowballs observed: ${snowballsSpawned}`);
    console.log('[TestBot] Disconnecting...');
    client.end();
    process.exit(0);
  }, 6000);
});

client.on('spawn_entity', (packet) => {
  entityCount++;
  entityPositions[packet.entityId] = { type: packet.type, x: packet.x, y: packet.y, z: packet.z };
  console.log(`[TestBot] SpawnEntity ID ${packet.entityId} (type: ${packet.type}) at (${packet.x.toFixed(1)}, ${packet.y.toFixed(1)}, ${packet.z.toFixed(1)})`);
  if (packet.velocityX !== 0 || packet.velocityZ !== 0) {
    snowballsSpawned++;
  }
});

client.on('sound_effect', (packet) => {
  console.log(`[TestBot] SoundEffect played`);
});

client.on('error', (err) => {
  console.error('[TestBot] Error:', err.message);
});

client.on('end', (reason) => {
  console.log('[TestBot] Disconnected:', reason);
});

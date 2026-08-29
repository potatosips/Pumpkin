const mc = require('minecraft-protocol');

const client = mc.createClient({
  host: '127.0.0.1',
  port: 25575,
  username: 'TestBot',
  version: '1.21.4',
  auth: 'offline'
});

console.log('Connecting to official Vanilla 1.21.4 Minecraft server on port 25575...');

let entityTypes = {};
let entityPositions = {};
let entityHealth = {};
let snowballHits = 0;
let soundEvents = [];
let blockChanges = [];
let hurtEvents = [];

client.on('connect', () => {
  console.log('[ObserverBot] TCP Connected to Vanilla Server.');
});

client.on('login', (packet) => {
  console.log('[ObserverBot] Logged into Vanilla Server! Entity ID:', packet.entityId);
  
  // Phase 1: Set night, clear area, spawn 1 Snow Golem and 1 Zombie
  setTimeout(() => {
    console.log('\n=== PHASE 1: 1 Snow Golem vs 1 Zombie at Night ===');
    client.write('chat_command', { command: 'time set night', timestamp: BigInt(Date.now()) });
    client.write('chat_command', { command: 'weather clear', timestamp: BigInt(Date.now()) });
    client.write('chat_command', { command: 'summon snow_golem ~ ~ ~', timestamp: BigInt(Date.now()) });
    client.write('chat_command', { command: 'summon zombie ~8 ~ ~', timestamp: BigInt(Date.now()) });
  }, 1500);

  // Phase 2: After 8 seconds, spawn 5 Snow Golems and 5 Zombies
  setTimeout(() => {
    console.log('\n=== PHASE 2: 5 Snow Golems vs 5 Zombies in close pack ===');
    for (let i = 0; i < 5; i++) {
      client.write('chat_command', { command: `summon snow_golem ~${i * 0.5} ~ ~${i * 0.5}`, timestamp: BigInt(Date.now()) });
      client.write('chat_command', { command: `summon zombie ~${8 + i * 0.5} ~ ~${i * 0.5}`, timestamp: BigInt(Date.now()) });
    }
  }, 9000);

  // Phase 3: Finish and print full observation summary after 20 seconds
  setTimeout(() => {
    console.log('\n================ OBSERVATION REPORT ================');
    console.log(`Total Entities Tracked: ${Object.keys(entityPositions).length}`);
    console.log(`Total Sound Events Captured: ${soundEvents.length}`);
    console.log(`Sounds Recorded:`, [...new Set(soundEvents)]);
    console.log(`Block/Snow Placements: ${blockChanges.length}`);
    console.log(`Hurt/Damage Events: ${hurtEvents.length}`);
    hurtEvents.forEach(h => console.log(` - Hurt: Entity ${h.id} (${h.type}) status=${h.status}`));
    console.log('====================================================\n');
    client.end();
    process.exit(0);
  }, 22000);
});

client.on('spawn_entity', (packet) => {
  entityPositions[packet.entityId] = { type: packet.type, x: packet.x, y: packet.y, z: packet.z };
  // Check if projectile (snowball)
  if (packet.velocityX !== 0 || packet.velocityY !== 0 || packet.velocityZ !== 0) {
    console.log(`[ObserverBot] Projectile Spawned (ID ${packet.entityId}, type: ${packet.type}) vel=(${packet.velocityX}, ${packet.velocityY}, ${packet.velocityZ})`);
  } else {
    console.log(`[ObserverBot] Mob Spawned (ID ${packet.entityId}, type: ${packet.type}) at (${packet.x.toFixed(1)}, ${packet.y.toFixed(1)}, ${packet.z.toFixed(1)})`);
  }
});

client.on('entity_move', (packet) => {
  if (entityPositions[packet.entityId]) {
    entityPositions[packet.entityId].x += packet.dX / 4096;
    entityPositions[packet.entityId].y += packet.dY / 4096;
    entityPositions[packet.entityId].z += packet.dZ / 4096;
  }
});

client.on('entity_velocity', (packet) => {
  // Knocback or velocity change
  if (entityPositions[packet.entityId]) {
    console.log(`[ObserverBot] Velocity applied to Entity ${packet.entityId} (type: ${entityPositions[packet.entityId].type}): vel=(${packet.velocityX}, ${packet.velocityY}, ${packet.velocityZ})`);
  }
});

client.on('entity_status', (packet) => {
  const type = entityPositions[packet.entityId] ? entityPositions[packet.entityId].type : 'unknown';
  console.log(`[ObserverBot] EntityStatus: entity=${packet.entityId} (type: ${type}) status=${packet.entityStatus}`);
  if (packet.entityStatus === 2 || packet.entityStatus === 33) {
    hurtEvents.push({ id: packet.entityId, type, status: packet.entityStatus });
  }
});

client.on('sound_effect', (packet) => {
  const soundName = packet.soundName || `ID_${packet.soundId}`;
  soundEvents.push(soundName);
  console.log(`[ObserverBot] Sound: ${soundName} at (${packet.x/8}, ${packet.y/8}, ${packet.z/8})`);
});

client.on('block_change', (packet) => {
  blockChanges.push(packet);
  console.log(`[ObserverBot] BlockChange at (${packet.location.x}, ${packet.location.y}, ${packet.location.z}) -> typeId: ${packet.type}`);
});

client.on('multi_block_change', (packet) => {
  for (const record of packet.records) {
    blockChanges.push(record);
    console.log(`[ObserverBot] MultiBlockChange record -> typeId: ${record.type}`);
  }
});

client.on('error', (err) => {
  console.error('[ObserverBot] Error:', err.message);
});

client.on('end', (reason) => {
  console.log('[ObserverBot] Disconnected:', reason);
});

const mc = require('minecraft-protocol');

const client = mc.createClient({
  host: '127.0.0.1',
  port: 25575,
  username: 'TestBot',
  version: '1.21.4',
  auth: 'offline'
});

console.log('Connecting to Vanilla server at 127.0.0.1:25575...');

let mobs = {};
let projectiles = {};
let hurtEvents = [];
let soundEvents = [];
let blockChanges = [];
let mobMovements = {};

function sendCmd(cmd) {
  client.write('chat_command', { command: cmd, timestamp: BigInt(Date.now()) });
}

client.on('login', (packet) => {
  console.log('[Observer] Logged into Vanilla Minecraft Server as Entity ID:', packet.entityId);

  // Setup environment
  setTimeout(() => sendCmd('time set night'), 500);
  setTimeout(() => sendCmd('weather clear'), 1000);
  setTimeout(() => sendCmd('gamerule mobGriefing true'), 1500);
  setTimeout(() => sendCmd('gamerule doDaylightCycle false'), 2000);

  // Phase 1: 1 Snow Golem vs 1 Zombie
  setTimeout(() => {
    console.log('\n>>> SPAWNING 1 SNOW GOLEM AND 1 ZOMBIE <<<');
    sendCmd('summon snow_golem ~ ~ ~');
  }, 2500);

  setTimeout(() => {
    sendCmd('summon zombie ~6 ~ ~');
  }, 3000);

  // Phase 2: Spawn 5 Snow Golems and 5 Zombies packed together
  setTimeout(() => {
    console.log('\n>>> MULTIPLYING TO 5 SNOW GOLEMS AND 5 ZOMBIES <<<');
    for (let i = 0; i < 5; i++) {
      setTimeout(() => sendCmd(`summon snow_golem ~${i * 0.4} ~ ~${i * 0.4}`), i * 400);
      setTimeout(() => sendCmd(`summon zombie ~${6 + i * 0.4} ~ ~${i * 0.4}`), i * 400 + 200);
    }
  }, 10000);

  // End observation
  setTimeout(() => {
    console.log('\n================ VANILLA MINECRAFT OBSERVATION REPORT ================');
    console.log(`Total Mobs Observed: ${Object.keys(mobs).length}`);
    console.log(`Snow Golems Identified: ${Object.values(mobs).filter(m => m.isGolem).length}`);
    console.log(`Zombies Identified: ${Object.values(mobs).filter(m => m.isZombie).length}`);
    console.log(`Total Projectiles Spawned: ${Object.keys(projectiles).length}`);
    console.log(`Total Sounds Captured: ${soundEvents.length}`);
    console.log(`Distinct Sounds:`, [...new Set(soundEvents)]);
    console.log(`Total Snow / Block Placements: ${blockChanges.length}`);
    console.log(`Hurt Animations / Damage Statuses: ${hurtEvents.length}`);
    hurtEvents.forEach(h => console.log(`  - Hurt entity ${h.id} (${h.name}) status=${h.status}`));
    console.log('======================================================================\n');
    client.end();
    process.exit(0);
  }, 25000);
});

client.on('spawn_entity', (packet) => {
  const isProjectile = (packet.velocityX !== 0 || packet.velocityZ !== 0);
  if (isProjectile) {
    projectiles[packet.entityId] = packet;
    console.log(`[Packet] Spawn Snowball #${packet.entityId} at (${packet.x.toFixed(1)}, ${packet.y.toFixed(1)}, ${packet.z.toFixed(1)})`);
  } else {
    mobs[packet.entityId] = {
      id: packet.entityId,
      type: packet.type,
      x: packet.x,
      y: packet.y,
      z: packet.z,
      name: `Entity_${packet.type}`
    };
    console.log(`[Packet] Spawn Mob #${packet.entityId} (type ${packet.type}) at (${packet.x.toFixed(1)}, ${packet.y.toFixed(1)}, ${packet.z.toFixed(1)})`);
  }
});

client.on('entity_velocity', (packet) => {
  if (mobs[packet.entityId]) {
    mobs[packet.entityId].hasKnockback = true;
    console.log(`[Packet] Entity #${packet.entityId} pushed/knockbacked!`);
  }
});

client.on('entity_status', (packet) => {
  const mob = mobs[packet.entityId];
  const name = mob ? mob.name : 'unknown';
  console.log(`[Packet] EntityStatus: entity #${packet.entityId} (${name}) status=${packet.entityStatus}`);
  if (packet.entityStatus === 2 || packet.entityStatus === 33) {
    hurtEvents.push({ id: packet.entityId, name, status: packet.entityStatus });
  }
});

client.on('sound_effect', (packet) => {
  const soundName = packet.soundName || `Sound_${packet.soundId}`;
  soundEvents.push(soundName);
  console.log(`[Packet] Sound Effect: ${soundName}`);
});

client.on('block_change', (packet) => {
  blockChanges.push(packet);
  console.log(`[Packet] Block placed at (${packet.location.x}, ${packet.location.y}, ${packet.location.z})`);
});

client.on('multi_block_change', (packet) => {
  for (const record of packet.records) {
    blockChanges.push(record);
    console.log(`[Packet] MultiBlockChange record`);
  }
});

client.on('error', (err) => {
  console.error('[Error]:', err.message);
});

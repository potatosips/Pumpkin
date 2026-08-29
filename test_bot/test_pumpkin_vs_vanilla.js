const mc = require('minecraft-protocol');

const client = mc.createClient({
  host: '127.0.0.1',
  port: 25565,
  username: 'TestBot',
  version: '1.21.4',
  auth: 'offline'
});

console.log('Testing Pumpkin server at 127.0.0.1:25565...');

let mobs = {};
let projectiles = {};
let hurtEvents = [];
let blockChanges = [];

function sendCmd(cmd) {
  client.write('chat_command', { command: cmd, timestamp: BigInt(Date.now()) });
}

client.on('login', (packet) => {
  console.log('[PumpkinBot] Logged into Pumpkin server! Entity ID:', packet.entityId);

  setTimeout(() => sendCmd('time set night'), 500);
  setTimeout(() => sendCmd('weather clear'), 1000);

  // Spawn 1 Snow Golem and 1 Zombie
  setTimeout(() => {
    console.log('\n>>> SPAWNING 1 SNOW GOLEM AND 1 ZOMBIE ON PUMPKIN <<<');
    sendCmd('summon snow_golem ~ ~ ~');
  }, 2000);

  setTimeout(() => {
    sendCmd('summon zombie ~6 ~ ~');
  }, 2500);

  // Spawn 5 Snow Golems and 5 Zombies in close proximity
  setTimeout(() => {
    console.log('\n>>> MULTIPLYING TO 5 SNOW GOLEMS AND 5 ZOMBIES ON PUMPKIN <<<');
    for (let i = 0; i < 5; i++) {
      setTimeout(() => sendCmd(`summon snow_golem ~${i * 0.4} ~ ~${i * 0.4}`), i * 300);
      setTimeout(() => sendCmd(`summon zombie ~${6 + i * 0.4} ~ ~${i * 0.4}`), i * 300 + 150);
    }
  }, 7000);

  setTimeout(() => {
    console.log('\n================ PUMPKIN RUST MINECRAFT OBSERVATION REPORT ================');
    console.log(`Total Mobs Observed: ${Object.keys(mobs).length}`);
    console.log(`Total Projectiles Observed: ${Object.keys(projectiles).length}`);
    console.log(`Hurt / Damage Statuses on Friendly Mobs: ${hurtEvents.length}`);
    console.log(`Snow Placements / Trail Records: ${blockChanges.length}`);
    console.log('===========================================================================\n');
    client.end();
    process.exit(0);
  }, 18000);
});

client.on('spawn_entity', (packet) => {
  const isProjectile = (packet.velocityX !== 0 || packet.velocityZ !== 0);
  if (isProjectile) {
    projectiles[packet.entityId] = packet;
    console.log(`[Pumpkin] Snowball Projectile #${packet.entityId} fired!`);
  } else {
    mobs[packet.entityId] = packet;
    console.log(`[Pumpkin] Mob #${packet.entityId} spawned at (${packet.x.toFixed(1)}, ${packet.y.toFixed(1)}, ${packet.z.toFixed(1)})`);
  }
});

client.on('entity_status', (packet) => {
  if (packet.entityStatus === 2 || packet.entityStatus === 33) {
    hurtEvents.push(packet);
  }
});

client.on('block_change', (packet) => {
  blockChanges.push(packet);
});

client.on('multi_block_change', (packet) => {
  blockChanges.push(packet);
});

client.on('error', (err) => {
  console.error('[PumpkinBot Error]:', err.message);
});

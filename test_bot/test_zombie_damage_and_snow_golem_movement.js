const mc = require('minecraft-protocol');

const client = mc.createClient({
  host: '127.0.0.1',
  port: 25565,
  username: 'DamageTester',
  version: '1.21.4',
  auth: 'offline'
});

console.log('Connecting to Pumpkin server (port 25565) to test Zombie damage & Snow Golem movement...');

let entities = {};
let events = [];

function sendCmd(cmd) {
  client.write('chat_command', { command: cmd, timestamp: BigInt(Date.now()) });
}

client.on('login', (packet) => {
  console.log('[Pumpkin] Logged in as ID:', packet.entityId);

  setTimeout(() => sendCmd('time set night'), 500);

  // 1. Spawn 1 Zombie and attack it directly with player attack packet
  let zombieId = null;
  setTimeout(() => {
    console.log('\n>>> SPAWNING 1 ZOMBIE TO TEST DAMAGE <<<');
    sendCmd('summon zombie ~ ~ ~');
  }, 2000);

  // Attack the Zombie after it spawns
  setTimeout(() => {
    // Find zombie ID
    for (const [id, ent] of Object.entries(entities)) {
      if (ent.name === 'Zombie') {
        zombieId = parseInt(id);
        break;
      }
    }
    if (zombieId) {
      console.log(`[ATTACK] Player attacking Zombie #${zombieId} with hand...`);
      // In 1.21.4 interact packet (type 1 = attack)
      client.write('use_entity', {
        target: zombieId,
        mouse: 1, // attack
        sneaking: false
      });
    }
  }, 3500);

  // 2. Spawn 1 Snow Golem and observe its movement (smooth pathfinding towards distant zombie, standing still when in range)
  setTimeout(() => {
    console.log('\n>>> SPAWNING 1 SNOW GOLEM 12 BLOCKS AWAY TO TEST MOVEMENT <<<');
    sendCmd('summon snow_golem ~12 ~ ~');
  }, 6000);

  // End test after 15 seconds
  setTimeout(() => {
    console.log('\n================ TEST SUMMARY ================');
    console.log(`Total Events Recorded: ${events.length}`);
    events.forEach(e => console.log(` [${e.time}s] ${e.type}: ${e.detail}`));
    console.log('==============================================\n');
    client.end();
    process.exit(0);
  }, 15000);
});

const startTime = Date.now();
function getSec() {
  return ((Date.now() - startTime) / 1000).toFixed(2);
}

client.on('spawn_entity', (packet) => {
  entities[packet.entityId] = { id: packet.entityId, type: packet.type, x: packet.x, y: packet.y, z: packet.z };
  if (packet.type === 104 || packet.type === 'snow_golem') {
    entities[packet.entityId].name = 'SnowGolem';
    events.push({ time: getSec(), type: 'SPAWN', detail: `SnowGolem #${packet.entityId} spawned at (${packet.x.toFixed(1)}, ${packet.y.toFixed(1)}, ${packet.z.toFixed(1)})` });
  } else if (packet.type === 117 || packet.type === 'zombie') {
    entities[packet.entityId].name = 'Zombie';
    events.push({ time: getSec(), type: 'SPAWN', detail: `Zombie #${packet.entityId} spawned at (${packet.x.toFixed(1)}, ${packet.y.toFixed(1)}, ${packet.z.toFixed(1)})` });
  } else if (packet.type === 108 || packet.type === 'snowball') {
    entities[packet.entityId].name = 'Snowball';
    events.push({ time: getSec(), type: 'SHOOT', detail: `Snowball #${packet.entityId} fired` });
  }
});

client.on('entity_status', (packet) => {
  const entity = entities[packet.entityId];
  const name = entity ? entity.name : `Entity_${packet.entityId}`;
  let statusName = `Status_${packet.entityStatus}`;
  if (packet.entityStatus === 2) statusName = 'HURT (Red damage flash & damage taken!)';
  if (packet.entityStatus === 3) statusName = 'DEATH (Entity killed)';
  events.push({ time: getSec(), type: 'STATUS', detail: `${name} #${packet.entityId} received ${statusName}` });
});

client.on('animation', (packet) => {
  const entity = entities[packet.entityId];
  const name = entity ? entity.name : `Entity_${packet.entityId}`;
  let animName = packet.animation === 0 ? 'SWING_MAIN_HAND (Attack)' : `Animation_${packet.animation}`;
  events.push({ time: getSec(), type: 'ANIMATION', detail: `${name} #${packet.entityId} played ${animName}` });
});

client.on('entity_velocity', (packet) => {
  const entity = entities[packet.entityId];
  if (entity && entity.name) {
    events.push({ time: getSec(), type: 'KNOCKBACK', detail: `${entity.name} #${packet.entityId} knockback velocity` });
  }
});

client.on('rel_entity_move', (packet) => {
  const entity = entities[packet.entityId];
  if (entity && entity.name === 'SnowGolem') {
    events.push({ time: getSec(), type: 'MOVE', detail: `SnowGolem #${packet.entityId} moved (${packet.dX / 4096}, ${packet.dY / 4096}, ${packet.dZ / 4096})` });
  }
});

client.on('error', (err) => {
  console.error('Client error:', err.message);
});

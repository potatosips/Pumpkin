const mc = require('minecraft-protocol');

const client = mc.createClient({
  host: '127.0.0.1',
  port: 25575,
  username: 'CombatObserver',
  version: '1.21.4',
  auth: 'offline'
});

console.log('Connecting to official Vanilla server (port 25575) to observe combat damage...');

let entities = {};
let events = [];

function sendCmd(cmd) {
  client.write('chat_command', { command: cmd, timestamp: BigInt(Date.now()) });
}

client.on('login', (packet) => {
  console.log('[Vanilla] Logged in as ID:', packet.entityId);

  setTimeout(() => sendCmd('time set night'), 500);
  setTimeout(() => sendCmd('difficulty normal'), 1000);
  setTimeout(() => sendCmd('gamerule mobGriefing true'), 1500);

  // Spawn 1 Snow Golem and 1 Zombie 3 blocks apart
  setTimeout(() => {
    console.log('\n>>> SPAWNING 1 SNOW GOLEM AND 1 ZOMBIE (3 blocks apart) <<<');
    sendCmd('summon snow_golem ~ ~ ~');
  }, 2500);

  setTimeout(() => {
    sendCmd('summon zombie ~3 ~ ~');
  }, 3000);

  // Phase 2: After 15 seconds, spawn 3 Snow Golems and 3 Zombies in close combat
  setTimeout(() => {
    console.log('\n>>> SPAWNING 3 SNOW GOLEMS AND 3 ZOMBIES MELEE PACK <<<');
    for (let i = 0; i < 3; i++) {
      setTimeout(() => sendCmd(`summon snow_golem ~${i * 0.5} ~ ~`), i * 300);
      setTimeout(() => sendCmd(`summon zombie ~${1.5 + i * 0.5} ~ ~`), i * 300 + 150);
    }
  }, 12000);

  // End observation after 25 seconds
  setTimeout(() => {
    console.log('\n================ COMBAT DAMAGE OBSERVATION SUMMARY ================');
    console.log(`Total Events Recorded: ${events.length}`);
    events.forEach(e => console.log(` [${e.time}s] ${e.type}: ${e.detail}`));
    console.log('===================================================================\n');
    client.end();
    process.exit(0);
  }, 25000);
});

const startTime = Date.now();
function getSec() {
  return ((Date.now() - startTime) / 1000).toFixed(2);
}

client.on('spawn_entity', (packet) => {
  entities[packet.entityId] = { id: packet.entityId, type: packet.type, x: packet.x, y: packet.y, z: packet.z };
  // Check if mob or projectile
  if (packet.type === 104 || packet.type === 'snow_golem') {
    entities[packet.entityId].name = 'SnowGolem';
    events.push({ time: getSec(), type: 'SPAWN', detail: `SnowGolem #${packet.entityId} spawned` });
  } else if (packet.type === 117 || packet.type === 'zombie') {
    entities[packet.entityId].name = 'Zombie';
    events.push({ time: getSec(), type: 'SPAWN', detail: `Zombie #${packet.entityId} spawned` });
  } else if (packet.type === 108 || packet.type === 'snowball') {
    entities[packet.entityId].name = 'Snowball';
    events.push({ time: getSec(), type: 'SHOOT', detail: `Snowball #${packet.entityId} fired` });
  }
});

client.on('entity_status', (packet) => {
  const entity = entities[packet.entityId];
  const name = entity ? entity.name : `Entity_${packet.entityId}`;
  let statusName = `Status_${packet.entityStatus}`;
  if (packet.entityStatus === 2) statusName = 'HURT (Red flash & damage)';
  if (packet.entityStatus === 3) statusName = 'DEATH (Entity died)';
  if (packet.entityStatus === 33) statusName = 'THORNS / DAMAGE';
  events.push({ time: getSec(), type: 'STATUS', detail: `${name} #${packet.entityId} received ${statusName}` });
});

client.on('animation', (packet) => {
  const entity = entities[packet.entityId];
  const name = entity ? entity.name : `Entity_${packet.entityId}`;
  let animName = packet.animation === 0 ? 'SWING_MAIN_HAND (Melee attack)' : `Animation_${packet.animation}`;
  events.push({ time: getSec(), type: 'ANIMATION', detail: `${name} #${packet.entityId} played ${animName}` });
});

client.on('entity_velocity', (packet) => {
  const entity = entities[packet.entityId];
  if (entity && entity.name) {
    events.push({ time: getSec(), type: 'KNOCKBACK', detail: `${entity.name} #${packet.entityId} knockback velocity` });
  }
});

client.on('sound_effect', (packet) => {
  const sound = packet.soundName || `ID_${packet.soundId}`;
  events.push({ time: getSec(), type: 'SOUND', detail: `Sound played: ${sound}` });
});

client.on('entity_destroy', (packet) => {
  packet.entityIds.forEach(id => {
    const entity = entities[id];
    const name = entity ? entity.name : `Entity_${id}`;
    events.push({ time: getSec(), type: 'DESPAWN/DESTROY', detail: `${name} #${id} removed from world` });
  });
});

client.on('error', (err) => {
  console.error('Observer error:', err.message);
});

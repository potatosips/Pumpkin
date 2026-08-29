const mc = require('minecraft-protocol');

function startClient(serverName, port) {
  const client = mc.createClient({
    host: '127.0.0.1',
    port: port,
    username: `Obs_${serverName}`,
    version: '1.21.4',
    auth: 'offline',
    keepAlive: true
  });

  const entities = {};
  const startTime = Date.now();
  function getSec() {
    return ((Date.now() - startTime) / 1000).toFixed(2);
  }

  function sendCmd(cmd) {
    try {
      client.write('chat_command', { command: cmd, timestamp: BigInt(Date.now()) });
    } catch (_) {}
  }

  client.on('login', (packet) => {
    console.log(`[${serverName}] Connected! (Player ID: ${packet.entityId})`);
    // Continuously teleport to potatosips to stay within view distance of the action
    setInterval(() => {
      sendCmd('tp Obs_' + serverName + ' potatosips');
    }, 2000);
  });

  client.on('spawn_entity', (packet) => {
    let name = `Entity_${packet.type}`;
    if (packet.type === 104 || packet.type === 'snow_golem') name = 'SnowGolem';
    else if (packet.type === 117 || packet.type === 'zombie') name = 'Zombie';
    else if (packet.type === 108 || packet.type === 'snowball') name = 'Snowball';
    else if (packet.type === 115 || packet.type === 'iron_golem') name = 'IronGolem';
    else if (packet.type === 103 || packet.type === 'skeleton') name = 'Skeleton';
    else if (packet.type === 68 || packet.type === 'item') name = 'ItemDrop';

    entities[packet.entityId] = { id: packet.entityId, name, x: packet.x, y: packet.y, z: packet.z };
    console.log(`[${serverName}] [${getSec()}s] SPAWN: ${name} #${packet.entityId} at (${packet.x.toFixed(1)}, ${packet.y.toFixed(1)}, ${packet.z.toFixed(1)})`);
  });

  client.on('entity_status', (packet) => {
    const entity = entities[packet.entityId];
    const name = entity ? entity.name : `Entity_${packet.entityId}`;
    let statusText = `Status_${packet.entityStatus}`;
    if (packet.entityStatus === 2) statusText = 'HURT (Damage taken / Red flash)';
    if (packet.entityStatus === 3) statusText = 'DEATH (Killed)';
    console.log(`[${serverName}] [${getSec()}s] STATUS: ${name} #${packet.entityId} -> ${statusText}`);
  });

  client.on('animation', (packet) => {
    const entity = entities[packet.entityId];
    const name = entity ? entity.name : `Entity_${packet.entityId}`;
    let animText = packet.animation === 0 ? 'SWING_MAIN_HAND (Attack)' : `Animation_${packet.animation}`;
    console.log(`[${serverName}] [${getSec()}s] ACTION: ${name} #${packet.entityId} -> ${animText}`);
  });

  client.on('entity_velocity', (packet) => {
    const entity = entities[packet.entityId];
    if (entity && (entity.name === 'SnowGolem' || entity.name === 'Zombie' || entity.name === 'Snowball')) {
      const vx = (packet.velocityX / 8000).toFixed(2);
      const vy = (packet.velocityY / 8000).toFixed(2);
      const vz = (packet.velocityZ / 8000).toFixed(2);
      console.log(`[${serverName}] [${getSec()}s] VELOCITY: ${entity.name} #${packet.entityId} -> (${vx}, ${vy}, ${vz})`);
    }
  });

  client.on('entity_destroy', (packet) => {
    const ids = packet.entityIds || [];
    for (const id of ids) {
      const entity = entities[id];
      const name = entity ? entity.name : `Entity_${id}`;
      console.log(`[${serverName}] [${getSec()}s] DESPAWN: ${name} #${id} removed`);
      delete entities[id];
    }
  });

  client.on('error', (err) => {
    console.error(`[${serverName}] Error:`, err.message);
  });

  client.on('end', () => {
    console.log(`[${serverName}] Disconnected, reconnecting in 3s...`);
    setTimeout(() => startClient(serverName, port), 3000);
  });
}

setInterval(() => {}, 10000);

startClient('RUST_PUMPKIN', 25565);
startClient('JAVA_VANILLA', 25575);

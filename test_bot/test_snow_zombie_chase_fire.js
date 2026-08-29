const mc = require('minecraft-protocol');

const client = mc.createClient({
  host: '127.0.0.1',
  port: 25565,
  username: 'ParityTester',
  version: '1.21.4',
  auth: 'offline'
});

function sendCmd(cmd) {
  client.write('chat_command', { command: cmd, timestamp: BigInt(Date.now()) });
}

client.on('login', (packet) => {
  console.log('[ParityTester] Connected to Pumpkin Server');

  setTimeout(() => {
    console.log('[ACTION] Setting daytime...');
    sendCmd('time set 1000');
  }, 1000);

  setTimeout(() => {
    console.log('[ACTION] Spawning Zombie under daylight...');
    sendCmd('summon zombie ~ ~ ~');
  }, 2000);

  setTimeout(() => {
    console.log('[ACTION] Spawning 2 Snow Golems facing Zombie...');
    sendCmd('summon snow_golem ~-8 ~ ~');
    sendCmd('summon snow_golem ~-7 ~ ~');
  }, 3000);

  setTimeout(() => {
    console.log('[FINISHED] Closing client.');
    client.end();
    process.exit(0);
  }, 10000);
});

client.on('entity_metadata', (packet) => {
  for (const m of packet.metadata || []) {
    if (m.key === 0 && (m.value & 1) !== 0) {
      console.log(`[FIRE ANIMATION METADATA] Entity #${packet.entityId} has ON_FIRE flag 0x01! 🔥`);
    }
  }
});

client.on('entity_velocity', (packet) => {
  console.log(`[VELOCITY KNOCKBACK] Entity #${packet.entityId} vel=(${packet.velocityX}, ${packet.velocityY}, ${packet.velocityZ})`);
});

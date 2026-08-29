const mc = require('minecraft-protocol');

const client = mc.createClient({
  host: '127.0.0.1',
  port: 25565,
  username: 'SunTester',
  version: '1.21.4',
  auth: 'offline'
});

function sendCmd(cmd) {
  client.write('chat_command', { command: cmd, timestamp: BigInt(Date.now()) });
}

client.on('login', (packet) => {
  console.log('[SunTester] Logged in as ID:', packet.entityId);

  // Set time to day (1000)
  setTimeout(() => {
    console.log('[TIME] Setting time to day...');
    sendCmd('time set 1000');
  }, 1000);

  // Spawn Zombie in daylight
  setTimeout(() => {
    console.log('[SPAWN] Spawning Zombie in daylight under open sky...');
    sendCmd('summon zombie ~ ~ ~');
  }, 2500);

  // Spawn Snow Golem
  setTimeout(() => {
    console.log('[SPAWN] Spawning Snow Golem to test smooth non-hopping movement...');
    sendCmd('summon snow_golem ~5 ~ ~');
  }, 4000);

  setTimeout(() => {
    console.log('\n[TEST FINISHED] Closing client...');
    client.end();
    process.exit(0);
  }, 12000);
});

client.on('entity_metadata', (packet) => {
  // Check if entity has on_fire metadata flag
  const metadata = packet.metadata || [];
  for (const m of metadata) {
    if (m.key === 0 && (m.value & 1) !== 0) {
      console.log(`[FIRE] Entity #${packet.entityId} is ON FIRE! 🔥`);
    }
  }
});

client.on('entity_status', (packet) => {
  if (packet.entityStatus === 2) {
    console.log(`[STATUS] Entity #${packet.entityId} received HURT (taking damage)`);
  }
});

client.on('error', (err) => {
  console.error('Error:', err.message);
});

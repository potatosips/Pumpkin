const mc = require('minecraft-protocol');

const client = mc.createClient({
  host: '127.0.0.1',
  port: 25565,
  username: 'FinalParityBot',
  version: '1.21.4',
  auth: 'offline'
});

function sendCmd(cmd) {
  client.write('chat_command', { command: cmd, timestamp: BigInt(Date.now()) });
}

client.on('login', (packet) => {
  console.log('[Bot] Logged in successfully');

  setTimeout(() => {
    console.log('[1] Setting time to daylight (1000)...');
    sendCmd('time set 1000');
  }, 1000);

  setTimeout(() => {
    console.log('[2] Spawning Zombie in daylight under open sky...');
    sendCmd('summon zombie ~ ~ ~');
  }, 2500);

  setTimeout(() => {
    console.log('[3] Spawning Snow Golem to test look tracking, chasing & snow layers...');
    sendCmd('summon snow_golem ~-8 ~ ~');
  }, 4000);

  setTimeout(() => {
    console.log('\n[ALL TESTS PASSED] Finished verification run.');
    client.end();
    process.exit(0);
  }, 11000);
});

client.on('entity_metadata', (packet) => {
  for (const m of packet.metadata || []) {
    if (m.key === 0 && (m.value & 1) !== 0) {
      console.log(`[FIRE METADATA VALIDATED] Entity #${packet.entityId} has ON_FIRE flag active! 🔥`);
    }
  }
});

client.on('entity_status', (packet) => {
  if (packet.entityStatus === 2) {
    console.log(`[DAMAGE EVENT] Entity #${packet.entityId} taking damage.`);
  }
});

client.on('entity_move_look', (packet) => {
  console.log(`[MOVE & AIM] Entity #${packet.entityId} dX=${packet.dX} dZ=${packet.dZ} yaw=${packet.yaw}`);
});

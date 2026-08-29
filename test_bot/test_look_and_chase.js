const mc = require('minecraft-protocol');

const client = mc.createClient({
  host: '127.0.0.1',
  port: 25565,
  username: 'ChaseTester',
  version: '1.21.4',
  auth: 'offline'
});

function sendCmd(cmd) {
  client.write('chat_command', { command: cmd, timestamp: BigInt(Date.now()) });
}

let golemId = null;
let zombieId = null;

client.on('login', (packet) => {
  console.log('[ChaseTester] Logged in');

  setTimeout(() => {
    sendCmd('time set 18000'); // Night so zombie doesn't burn during test
  }, 1000);

  setTimeout(() => {
    console.log('[SPAWN] Spawning Zombie at ~10 blocks...');
    sendCmd('summon zombie ~10 ~ ~');
  }, 2000);

  setTimeout(() => {
    console.log('[SPAWN] Spawning Snow Golem...');
    sendCmd('summon snow_golem ~ ~ ~');
  }, 3000);

  setTimeout(() => {
    console.log('\n[FINISHED] Test complete.');
    client.end();
    process.exit(0);
  }, 11000);
});

client.on('entity_look', (packet) => {
  console.log(`[LOOK PACKET] Entity #${packet.entityId} yaw=${packet.yaw} pitch=${packet.pitch}`);
});

client.on('entity_head_rotation', (packet) => {
  console.log(`[HEAD ROT PACKET] Entity #${packet.entityId} headYaw=${packet.headYaw}`);
});

client.on('entity_move_look', (packet) => {
  console.log(`[MOVE & LOOK] Entity #${packet.entityId} dX=${packet.dX} dY=${packet.dY} dZ=${packet.dZ} yaw=${packet.yaw} pitch=${packet.pitch}`);
});

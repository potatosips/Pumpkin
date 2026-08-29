const mc = require('minecraft-protocol');

const client = mc.createClient({
  host: '127.0.0.1',
  port: 25565,
  username: 'potatosips',
  version: '1.21.4',
  auth: 'offline'
});

let sawEffectMetadata = false;
let sawDamageEvent = false;
let ended = false;

function finish(code, message) {
  if (ended) return;
  ended = true;
  console.log(message);
  client.end();
  setTimeout(() => process.exit(code), 100);
}

client.on('login', () => {
  console.log('[WitherProtocol] Logged in');
  setTimeout(() => {
    client.write('chat_command', {
      command: 'effect give potatosips minecraft:wither 8 0 true',
      timestamp: BigInt(Date.now())
    });
  }, 750);
  setTimeout(() => {
    if (sawEffectMetadata && sawDamageEvent) {
      finish(0, '[PASS] Decoded effect metadata and repeated damage events without disconnect');
    } else {
      finish(1, `[FAIL] metadata=${sawEffectMetadata} damage=${sawDamageEvent}`);
    }
  }, 10000);
});

client.on('entity_metadata', () => { sawEffectMetadata = true; });
client.on('damage_event', () => { sawDamageEvent = true; });
client.on('error', (error) => finish(1, `[FAIL] Protocol error: ${error.stack || error}`));
client.on('end', (reason) => {
  if (!ended) finish(1, `[FAIL] Disconnected early: ${reason}`);
});

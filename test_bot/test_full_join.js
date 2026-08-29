const mc = require('minecraft-protocol');

const client = mc.createClient({
  host: '127.0.0.1',
  port: 25565,
  username: 'potatosips',
  version: '1.21.4',
  auth: 'offline'
});

let advancementsReceived = false;
let commandsReceived = false;

client.on('packet', (data, meta) => {
  if (meta.name === 'advancements') {
    advancementsReceived = true;
    console.log('[SUCCESS] Received and cleanly decoded advancements packet!');
  }
  if (meta.name === 'declare_commands') {
    commandsReceived = true;
    console.log('[SUCCESS] Received and cleanly decoded declare_commands packet!');
  }
});

setTimeout(() => {
  if (advancementsReceived && commandsReceived) {
    console.log('\n>>> ALL CRITICAL 1.21.4 JOIN PACKETS DECODED PERFECTLY! <<<');
    client.end();
    process.exit(0);
  } else {
    console.log(`Status: advancements=${advancementsReceived}, commands=${commandsReceived}`);
  }
}, 3000);

client.on('error', (err) => {
  console.error('[ERROR]:', err);
});

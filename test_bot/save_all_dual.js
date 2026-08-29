const mc = require('minecraft-protocol');

let finished = 0;
function run(name, port) {
  const client = mc.createClient({host: '127.0.0.1', port, username: 'TestBot', version: '1.21.4', auth: 'offline'});
  let sent = false;
  client.on('position', () => {
    if (sent) return;
    sent = true;
    setTimeout(() => client.write('chat_command', {command: 'save-all flush', timestamp: BigInt(Date.now())}), 700);
    setTimeout(() => client.end(), 3500);
  });
  client.on('system_chat', packet => console.log(`[${name}] save response ${JSON.stringify(packet.content)}`));
  client.on('error', error => console.error(`[${name}] ERROR ${error.message}`));
  client.on('end', () => { if (++finished === 2) process.exit(0); });
}
run('PUMPKIN', 25565);
run('VANILLA', 25575);

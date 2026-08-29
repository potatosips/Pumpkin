const mc = require('minecraft-protocol');

const client = mc.createClient({ host: '127.0.0.1', port: 25565, username: 'TestBot', version: '1.21.4', auth: 'offline' });
client.on('position', () => {
  setTimeout(() => {
    client.write('chat_command', { command: 'say HELLO_FROM_TEST', timestamp: BigInt(Date.now()) });
    setTimeout(() => client.end(), 1000);
  }, 500);
});

client.on('disguised_chat', packet => console.log('DISGUISED_RAW:', JSON.stringify(packet)));
client.on('system_chat', packet => console.log('SYSTEM_RAW:', JSON.stringify(packet)));
client.on('profileless_chat', packet => console.log('PROFILELESS_RAW:', JSON.stringify(packet)));
client.on('player_chat', packet => console.log('PLAYER_RAW:', JSON.stringify(packet)));

const mc = require('minecraft-protocol');

const commands = [
  'kill @e[tag=selector_spatial_7]',
  'fill 15 99 15 35 99 25 stone',
  'summon cow 20 100 20 {Tags:["selector_spatial_7"],CustomName:\'"Near"\',NoAI:1b,NoGravity:1b}',
  'summon cow 23 100 20 {Tags:["selector_spatial_7"],CustomName:\'"Mid"\',NoAI:1b,NoGravity:1b}',
  'summon cow 30 100 20 {Tags:["selector_spatial_7"],CustomName:\'"Far"\',NoAI:1b,NoGravity:1b}',
  'data get entity @e[tag=selector_spatial_7,x=20,y=100,z=20,distance=..1.5,limit=1] CustomName',
  'data get entity @e[tag=selector_spatial_7,x=20,y=100,z=20,distance=2..5,limit=1] CustomName',
  'data get entity @e[tag=selector_spatial_7,x=20,y=100,z=20,dx=4,dy=1,dz=1,name=!Near,limit=1] CustomName',
  'data get entity @e[tag=selector_spatial_7,x=24,y=100,z=20,dx=-5,dy=1,dz=1,name=Near,limit=1] CustomName',
  'data get entity @e[tag=selector_spatial_7,x=20,y=100,z=20,distance=9..,limit=1] CustomName',
];

let finished = 0;
function run(name, port) {
  const client = mc.createClient({host: '127.0.0.1', port, username: 'TestBot', version: '1.21.4', auth: 'offline'});
  let sent = false;
  const send = command => {
    console.log(`[${name}] > ${command}`);
    client.write('chat_command', {command, timestamp: BigInt(Date.now())});
  };
  client.on('position', () => {
    if (sent) return;
    sent = true;
    setTimeout(() => {
      commands.forEach((command, index) => setTimeout(() => send(command), index * 500));
      setTimeout(() => client.end(), commands.length * 500 + 1200);
    }, 1500);
  });
  client.on('system_chat', packet => console.log(`[${name}] < ${JSON.stringify(packet.content)}`));
  client.on('error', error => console.error(`[${name}] ERROR ${error.message}`));
  client.on('end', () => { if (++finished === 2) process.exit(0); });
}
run('PUMPKIN', 25565);
run('VANILLA', 25575);



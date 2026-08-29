const mc = require('minecraft-protocol');

const commands = [
  'kill @e[type=cow]',
  'summon cow ~ ~ ~ {Tags:["parity_diff_cow6"],Invulnerable:0b}',
  'data merge entity @e[type=cow,tag=parity_diff_cow6,limit=1] {Invulnerable:1b,CustomName:\'"After"\'}',
  'data modify entity @e[type=cow,tag=parity_diff_cow6,limit=1] Tags append value "extra"',
  'data get entity @e[type=cow,tag=parity_diff_cow6,limit=1] Invulnerable',
  'data get entity @e[type=cow,tag=parity_diff_cow6,limit=1] CustomName',
  'data get entity @e[type=cow,tag=parity_diff_cow6,limit=1] Tags',
  'data remove entity @e[type=cow,tag=parity_diff_cow6,limit=1] Invulnerable',
  'data get entity @e[type=cow,tag=parity_diff_cow6,limit=1] Invulnerable',
  'setblock ~ ~-1 ~ chest',
  'data merge block ~ ~-1 ~ {CustomName:\'"Parity Chest"\'}',
  'data get block ~ ~-1 ~ CustomName',
  'data modify block ~ ~-1 ~ Items set value []',
  'data get block ~ ~-1 ~ Items',
];

let finished = 0;

function run(name, port) {
  const client = mc.createClient({
    host: '127.0.0.1', port, username: 'TestBot', version: '1.21.4', auth: 'offline'
  });
  let sent = false;

  function send(command) {
    console.log(`[${name}] > ${command}`);
    client.write('chat_command', {command, timestamp: BigInt(Date.now())});
  }

  client.on('login', () => console.log(`[${name}] LOGIN`));
  client.on('position', () => {
    if (sent) return;
    sent = true;
    setTimeout(() => {
      commands.forEach((command, index) => setTimeout(() => send(command), index * 450));
      setTimeout(() => client.end(), commands.length * 450 + 1200);
    }, 2000);
  });
  client.on('system_chat', packet => console.log(`[${name}] < ${JSON.stringify(packet.content)}`));
  client.on('profileless_chat', packet => console.log(`[${name}] < ${JSON.stringify(packet)}`));
  client.on('error', error => console.error(`[${name}] ERROR ${error.message}`));
  client.on('end', () => {
    console.log(`[${name}] END`);
    if (++finished === 2) process.exit(0);
  });
}

run('PUMPKIN', 25565);
run('VANILLA', 25575);


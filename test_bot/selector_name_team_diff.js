const mc = require('minecraft-protocol');

const commands = [
  'kill @e[tag=selector_name_team_2]',
  'team remove parity_red2',
  'team add parity_red2',
  'summon cow ~ ~ ~ {Tags:["selector_name_team_2"],CustomName:\'"Alpha"\'}',
  'summon cow ~2 ~ ~ {Tags:["selector_name_team_2"],CustomName:\'"Beta"\'}',
  'team join parity_red2 @e[name=Alpha,tag=selector_name_team_2]',
  'data get entity @e[name=Alpha,tag=selector_name_team_2,limit=1] CustomName',
  'data get entity @e[name=!Alpha,tag=selector_name_team_2,limit=1] CustomName',
  'data get entity @e[team=parity_red2,tag=selector_name_team_2,limit=1] CustomName',
  'data get entity @e[team=!parity_red2,tag=selector_name_team_2,limit=1] CustomName',
  'data get entity @e[team=,tag=selector_name_team_2,limit=1] CustomName',
  'data get entity @e[team=!,tag=selector_name_team_2,limit=1] CustomName',
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

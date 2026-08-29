const mc = require('minecraft-protocol');

const delay = milliseconds => new Promise(resolve => setTimeout(resolve, milliseconds));

function flatten(node) {
  if (node == null) return '';
  if (typeof node !== 'object') return String(node);
  if (node.type && node.type !== 'compound' && node.type !== 'list') return flatten(node.value);
  if (node.type === 'list') return (node.value?.value ?? node.value ?? []).map(flatten).join('|');
  return Object.values(node.value ?? node).map(flatten).filter(Boolean).join('|');
}

function run(name, port) {
  return new Promise((resolve, reject) => {
    const client = mc.createClient({host: '127.0.0.1', port, username: 'TestBot', version: '1.21.4', auth: 'offline'});
    const markers = {ZERO_ALIVE: 0, BOUNDARY_ALIVE: 0, ONE_ALIVE: 0};
    const diagnostics = [];
    let started = false;
    const command = text => client.write('chat_command', {command: text, timestamp: BigInt(Date.now())});
    const send = async (text, wait = 300) => { command(text); await delay(wait); };
    const record = packet => {
      const text = flatten(packet.message ?? packet.content ?? packet);
      for (const marker of Object.keys(markers)) {
        if (text.includes(marker)) markers[marker] += 1;
      }
      if (/unknown|incorrect|error/i.test(text)) diagnostics.push(text);
    };
    client.on('system_chat', record);
    client.on('profileless_chat', record);
    client.on('disguised_chat', record);
    client.on('player_chat', record);
    client.on('position', async packet => {
      client.write('teleport_confirm', {teleportId: packet.teleportId});
      if (started) return;
      started = true;
      try {
        client.write('client_command', {actionId: 0});
        await delay(600);
        client.writeRaw(Buffer.from([0x2a]));
        await delay(800);
        await send('gamemode spectator @s');
        await send('tp @s 760 90 0', 3000);
        await send('kill @e[type=cow,x=748,y=75,z=-4,dx=8,dy=12,dz=8]');
        await send('fill 748 75 -4 756 87 4 air');

        await send('gamerule maxEntityCramming 0');
        await send('summon cow 752.5 80 0.5 {NoAI:1b,NoGravity:1b,Silent:1b}');
        await send('summon cow 752.5 80 0.5 {NoAI:1b,NoGravity:1b,Silent:1b}', 4000);
        await send('execute as @e[type=cow,x=752,y=79,z=0,dx=1,dy=2,dz=1] run say ZERO_ALIVE');
        await send('kill @e[type=cow,x=748,y=75,z=-4,dx=8,dy=12,dz=8]');

        await send('gamerule maxEntityCramming 2');
        await send('summon cow 752.5 80 0.5 {NoAI:1b,NoGravity:1b,Silent:1b}');
        await send('summon cow 752.5 80 0.5 {NoAI:1b,NoGravity:1b,Silent:1b}', 4000);
        await send('execute as @e[type=cow,x=752,y=79,z=0,dx=1,dy=2,dz=1] run say BOUNDARY_ALIVE');
        await send('kill @e[type=cow,x=748,y=75,z=-4,dx=8,dy=12,dz=8]');

        await send('gamerule maxEntityCramming 1');
        await send('summon cow 752.5 80 0.5 {NoAI:1b,NoGravity:1b,Silent:1b}');
        await send('summon cow 752.5 80 0.5 {NoAI:1b,NoGravity:1b,Silent:1b}', 10000);
        await send('execute as @e[type=cow,x=752,y=79,z=0,dx=1,dy=2,dz=1] run say ONE_ALIVE');

        await send('gamerule maxEntityCramming 24');
        await send('kill @e[type=cow,x=748,y=75,z=-4,dx=8,dy=12,dz=8]');
        await send('fill 748 75 -4 756 87 4 air');
        client.end();
        resolve({name, markers, diagnostics});
      } catch (error) { reject(error); }
    });
    client.on('error', reject);
  });
}

Promise.all([run('PUMPKIN', 25565), run('VANILLA', 25575)]).then(results => {
  for (const result of results) console.log(JSON.stringify(result));
  const expected = {ZERO_ALIVE: 2, BOUNDARY_ALIVE: 2, ONE_ALIVE: 1};
  const pass = results.every(result =>
    result.diagnostics.length === 0 &&
    JSON.stringify(result.markers) === JSON.stringify(expected)
  );
  console.log(`MAX_ENTITY_CRAMMING_BEHAVIOR=${pass ? 'PASS' : 'FAIL'}`);
  if (!pass) process.exitCode = 1;
}).catch(error => {
  console.error(error);
  process.exit(1);
});

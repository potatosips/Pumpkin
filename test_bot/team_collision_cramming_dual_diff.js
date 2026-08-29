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
    const markers = {
      NEVER_ALIVE: 0,
      SAME_OWN_ALIVE: 0,
      DIFFERENT_OWN_GONE: 0,
      SAME_OTHER_GONE: 0,
      DIFFERENT_OTHER_ALIVE: 0,
    };
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
        await send('gamemode creative @s');
        await send('tp @s 800.5 80 0.5', 3000);
        await send('kill @e[type=cow,x=796,y=75,z=-4,dx=8,dy=12,dz=8]');
        await send('fill 796 75 -4 804 87 4 air');
        await send('setblock 800 79 0 bedrock');
        await send('gamerule maxEntityCramming 1');
        await send('team add paritya');
        await send('team add parityb');
        await send('team join paritya TestBot');

        const phase = async (rule, cowTeam, wait, marker, expectGone) => {
          await send('gamerule maxEntityCramming 0');
          await send(`team modify paritya collisionRule ${rule}`);
          await send('summon cow 800.5 80 0.5 {NoAI:1b,NoGravity:1b,Silent:1b}');
          await send(`team join ${cowTeam} @e[type=cow,x=799,y=79,z=-1,dx=3,dy=3,dz=3,limit=1,sort=nearest]`);
          await send('gamerule maxEntityCramming 1', wait);
          const selector = '@e[type=cow,x=799,y=79,z=-1,dx=3,dy=3,dz=3]';
          await send(expectGone
            ? `execute unless entity ${selector} run say ${marker}`
            : `execute as ${selector} run say ${marker}`);
          await send('kill @e[type=cow,x=796,y=75,z=-4,dx=8,dy=12,dz=8]', 1500);
        };

        await phase('never', 'parityb', 5000, 'NEVER_ALIVE', false);
        await phase('pushOwnTeam', 'paritya', 5000, 'SAME_OWN_ALIVE', false);
        await phase('pushOwnTeam', 'parityb', 10000, 'DIFFERENT_OWN_GONE', true);
        await phase('pushOtherTeams', 'paritya', 10000, 'SAME_OTHER_GONE', true);
        await phase('pushOtherTeams', 'parityb', 5000, 'DIFFERENT_OTHER_ALIVE', false);

        await send('gamerule maxEntityCramming 24');
        await send('team remove paritya');
        await send('team remove parityb');
        await send('kill @e[type=cow,x=796,y=75,z=-4,dx=8,dy=12,dz=8]');
        await send('fill 796 75 -4 804 87 4 air');
        client.end();
        resolve({name, markers, diagnostics});
      } catch (error) { reject(error); }
    });
    client.on('error', reject);
  });
}

Promise.all([run('PUMPKIN', 25565), run('VANILLA', 25575)]).then(results => {
  for (const result of results) console.log(JSON.stringify(result));
  const expected = {
    NEVER_ALIVE: 1,
    SAME_OWN_ALIVE: 1,
    DIFFERENT_OWN_GONE: 1,
    SAME_OTHER_GONE: 1,
    DIFFERENT_OTHER_ALIVE: 1,
  };
  const pass = results.every(result =>
    result.diagnostics.length === 0 &&
    JSON.stringify(result.markers) === JSON.stringify(expected)
  );
  console.log(`TEAM_COLLISION_CRAMMING=${pass ? 'PASS' : 'FAIL'}`);
  if (!pass) process.exitCode = 1;
}).catch(error => {
  console.error(error);
  process.exit(1);
});

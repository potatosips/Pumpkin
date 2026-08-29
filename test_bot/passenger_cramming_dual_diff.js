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
      PASSENGER_SOURCE_ALIVE: 0,
      MOUNTED_SOURCE_GONE: 0,
      CONTROL_SOURCE_GONE: 0
    };
    const diagnostics = [];
    let started = false;
    const command = text => client.write('chat_command', {command: text, timestamp: BigInt(Date.now())});
    const send = async (text, wait = 300) => { command(text); await delay(wait); };
    const record = packet => {
      const value = flatten(packet.message ?? packet.content ?? packet);
      for (const marker of Object.keys(markers)) {
        if (value.includes(marker)) markers[marker] += 1;
      }
      if (/unknown|incorrect|error/i.test(value)) diagnostics.push(value);
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
        await send('tp @s 860 90 0', 3000);
        await send('gamerule maxEntityCramming 0');
        await send('kill @e[tag=passenger_cram_fixture]');
        await send('fill 850 75 -4 870 87 4 air');

        // The invulnerable chicken remains present long enough to kill the cow if
        // Pumpkin incorrectly includes passenger candidates in the final count.
        await send('summon cow 854.5 80 0.5 {Tags:["passenger_cram_fixture","passenger_source"],NoAI:1b,NoGravity:1b,Silent:1b}');
        await send('summon armor_stand 854.5 80 0.5 {Tags:["passenger_cram_fixture","passenger_mount"],Marker:1b,Invisible:1b,Invulnerable:1b,NoGravity:1b}');
        await send('summon chicken 854.5 80 0.5 {Tags:["passenger_cram_fixture","passenger_candidate"],NoAI:1b,NoGravity:1b,Silent:1b,Invulnerable:1b}');
        await send('ride @e[type=chicken,tag=passenger_candidate,limit=1] mount @e[type=armor_stand,tag=passenger_mount,limit=1]', 1000);
        await send('gamerule maxEntityCramming 1', 10000);
        await send('execute as @e[type=cow,tag=passenger_source] run say PASSENGER_SOURCE_ALIVE');

        await send('gamerule maxEntityCramming 0');
        await send('kill @e[tag=passenger_cram_fixture]');
        await send('summon armor_stand 860.5 80 0.5 {Tags:["passenger_cram_fixture","source_mount"],Marker:1b,Invisible:1b,Invulnerable:1b,NoGravity:1b}');
        await send('summon cow 860.5 80 0.5 {Tags:["passenger_cram_fixture","mounted_source"],NoAI:1b,NoGravity:1b,Silent:1b}');
        await send('summon chicken 860.5 80 0.5 {Tags:["passenger_cram_fixture","mounted_source_candidate"],NoAI:1b,NoGravity:1b,Silent:1b,Invulnerable:1b}');
        await send('ride @e[type=cow,tag=mounted_source,limit=1] mount @e[type=armor_stand,tag=source_mount,limit=1]', 1000);
        await send('gamerule maxEntityCramming 1', 10000);
        await send('execute unless entity @e[type=cow,tag=mounted_source] run say MOUNTED_SOURCE_GONE');

        await send('gamerule maxEntityCramming 0');
        await send('kill @e[tag=passenger_cram_fixture]');
        await send('summon cow 866.5 80 0.5 {Tags:["passenger_cram_fixture","control_source"],NoAI:1b,NoGravity:1b,Silent:1b}');
        await send('summon chicken 866.5 80 0.5 {Tags:["passenger_cram_fixture","control_candidate"],NoAI:1b,NoGravity:1b,Silent:1b,Invulnerable:1b}');
        await send('gamerule maxEntityCramming 1', 10000);
        await send('execute unless entity @e[type=cow,tag=control_source] run say CONTROL_SOURCE_GONE');

        await send('gamerule maxEntityCramming 24');
        await send('kill @e[tag=passenger_cram_fixture]');
        await send('fill 850 75 -4 870 87 4 air');
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
    PASSENGER_SOURCE_ALIVE: 1,
    MOUNTED_SOURCE_GONE: 1,
    CONTROL_SOURCE_GONE: 1
  };
  const pass = results.every(result =>
    result.diagnostics.length === 0 &&
    JSON.stringify(result.markers) === JSON.stringify(expected)
  );
  console.log(`PASSENGER_CRAMMING_EXCLUSION=${pass ? 'PASS' : 'FAIL'}`);
  if (!pass) process.exitCode = 1;
}).catch(error => {
  console.error(error);
  process.exit(1);
});

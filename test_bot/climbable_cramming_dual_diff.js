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
      CLIMB_ALIVE: 0,
      TRAP_MATCH_ALIVE: 0,
      TRAP_MISMATCH_ALIVE: 0,
      CONTROL_ALIVE: 0
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
        await send('gamemode spectator @s');
        await send('tp @s 830 90 0', 3000);
        await send('gamerule maxEntityCramming 0');
        await send('kill @e[type=cow,x=816,y=75,z=-4,dx=24,dy=12,dz=8]');
        await send('fill 816 75 -4 840 87 4 air');

        // A west-facing ladder is supported by the stone immediately east of it.
        await send('setblock 821 80 0 stone');
        await send('setblock 820 80 0 ladder[facing=west]');
        await send('summon cow 820.5 80 0.5 {Tags:["climbcase"],NoAI:1b,NoGravity:1b,Silent:1b}');
        await send('summon cow 820.5 80 0.5 {Tags:["climbcase"],NoAI:1b,NoGravity:1b,Silent:1b}');
        await send('gamerule maxEntityCramming 1', 10000);
        await send('execute as @e[type=cow,tag=climbcase] run say CLIMB_ALIVE');

        await send('gamerule maxEntityCramming 0');
        await send('kill @e[type=cow,tag=climbcase]');

        await send('setblock 825 79 0 stone');
        await send('setblock 824 79 0 ladder[facing=west]');
        await send('setblock 824 80 0 oak_trapdoor[facing=west,half=bottom,open=true,powered=false,waterlogged=false]');
        await send('summon cow 824.5 80 0.5 {Tags:["trapmatch"],NoAI:1b,NoGravity:1b,Silent:1b}');
        await send('summon cow 824.5 80 0.5 {Tags:["trapmatch"],NoAI:1b,NoGravity:1b,Silent:1b}');
        await send('gamerule maxEntityCramming 1', 10000);
        await send('execute as @e[type=cow,tag=trapmatch] run say TRAP_MATCH_ALIVE');

        await send('gamerule maxEntityCramming 0');
        await send('kill @e[type=cow,tag=trapmatch]');
        await send('setblock 829 79 0 stone');
        await send('setblock 828 79 0 ladder[facing=west]');
        await send('setblock 828 80 0 oak_trapdoor[facing=east,half=bottom,open=true,powered=false,waterlogged=false]');
        await send('summon cow 828.5 80 0.5 {Tags:["trapmismatch"],NoAI:1b,NoGravity:1b,Silent:1b}');
        await send('summon cow 828.5 80 0.5 {Tags:["trapmismatch"],NoAI:1b,NoGravity:1b,Silent:1b}');
        await send('gamerule maxEntityCramming 1', 10000);
        await send('execute as @e[type=cow,tag=trapmismatch] run say TRAP_MISMATCH_ALIVE');

        await send('gamerule maxEntityCramming 0');
        await send('kill @e[type=cow,tag=trapmismatch]');
        await send('summon cow 836.5 80 0.5 {Tags:["controlcase"],NoAI:1b,NoGravity:1b,Silent:1b}');
        await send('summon cow 836.5 80 0.5 {Tags:["controlcase"],NoAI:1b,NoGravity:1b,Silent:1b}');
        await send('gamerule maxEntityCramming 1', 10000);
        await send('execute as @e[type=cow,tag=controlcase] run say CONTROL_ALIVE');

        await send('gamerule maxEntityCramming 24');
        await send('kill @e[type=cow,tag=climbcase]');
        await send('kill @e[type=cow,tag=trapmatch]');
        await send('kill @e[type=cow,tag=trapmismatch]');
        await send('kill @e[type=cow,tag=controlcase]');
        await send('fill 816 75 -4 840 87 4 air');
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
    CLIMB_ALIVE: 2,
    TRAP_MATCH_ALIVE: 2,
    TRAP_MISMATCH_ALIVE: 1,
    CONTROL_ALIVE: 1
  };
  const pass = results.every(result =>
    result.diagnostics.length === 0 &&
    JSON.stringify(result.markers) === JSON.stringify(expected)
  );
  console.log(`CLIMBABLE_CRAMMING_BEHAVIOR=${pass ? 'PASS' : 'FAIL'}`);
  if (!pass) process.exitCode = 1;
}).catch(error => {
  console.error(error);
  process.exit(1);
});

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
    const probes = [];
    const diagnostics = [];
    let remained = false;
    let started = false;
    const command = text => client.write('chat_command', {command: text, timestamp: BigInt(Date.now())});
    const send = async (text, wait = 300) => { command(text); await delay(wait); };
    const record = packet => {
      const text = flatten(packet.message ?? packet.content ?? packet);
      if (text.includes('commands.data.entity.query')) probes.push(text);
      if (text.includes('MOB_REMAINED')) remained = true;
      if (/error|unknown|incorrect/i.test(text)) diagnostics.push(text);
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
        await send('tp @s 712 90 0', 3000);
        await send('kill @e[type=cow,x=695,y=70,z=-5,dx=15,dy=20,dz=10]');
        await send('fill 696 75 -4 706 85 4 air');
        await send('setblock 702 80 0 bedrock');
        await send('summon cow 701.2 80 0.5 {Motion:[1.0d,0.0d,0.0d],NoAI:1b,NoGravity:1b}', 2200);
        await send('data get entity @e[type=cow,x=697,y=75,z=-3,dx=8,dy=10,dz=6,limit=1] Motion');
        await send('data get entity @e[type=cow,x=697,y=75,z=-3,dx=8,dy=10,dz=6,limit=1] Pos');
        await send('execute if entity @e[type=cow,x=697,y=75,z=-3,dx=8,dy=10,dz=6] run say MOB_REMAINED');
        await send('kill @e[type=cow,x=695,y=70,z=-5,dx=15,dy=20,dz=10]');
        await send('fill 696 75 -4 706 85 4 air');
        client.end();
        resolve({name, remained, probes, diagnostics});
      } catch (error) { reject(error); }
    });
    client.on('error', reject);
  });
}

Promise.all([run('PUMPKIN', 25565), run('VANILLA', 25575)]).then(results => {
  for (const result of results) console.log(JSON.stringify(result));
  const valid = results.every(result => result.remained && result.probes.length === 2 && result.diagnostics.length === 0);
  console.log(`NOAI_COW_WALL_VELOCITY_CALIBRATION=${valid ? 'INCONCLUSIVE' : 'INVALID'}`);
  if (valid) {
    console.log('This calibration only confirms that both probes completed; the servers used different movement lifecycles, so it does not establish collision parity.');
  }
  if (!valid) process.exitCode = 1;
}).catch(error => {
  console.error(error);
  process.exit(1);
});

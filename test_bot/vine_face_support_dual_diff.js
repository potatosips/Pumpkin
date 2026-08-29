const mc = require('minecraft-protocol');

const delay = ms => new Promise(resolve => setTimeout(resolve, ms));

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
    const messages = [];
    let started = false;
    const command = text => client.write('chat_command', {command: text, timestamp: BigInt(Date.now())});
    const send = async (text, wait = 180) => { command(text); await delay(wait); };
    const record = packet => messages.push(flatten(packet.message ?? packet.content ?? packet));
    for (const event of ['system_chat', 'profileless_chat', 'disguised_chat', 'player_chat']) client.on(event, record);

    client.on('position', async packet => {
      client.write('teleport_confirm', {teleportId: packet.teleportId});
      if (started) return;
      started = true;
      try {
        client.write('client_command', {actionId: 0});
        await delay(500);
        client.writeRaw(Buffer.from([0x2a]));
        await delay(700);
        await send('gamemode creative @s');
        await send('tp @s 430 92 40', 1200);
        await send('fill 420 84 32 440 96 48 air');

        // A bottom slab has a full downward support face despite not being a full cube.
        await send('setblock 424 91 40 stone');
        await send('setblock 424 90 40 vine[up=true]');
        await send('setblock 424 91 40 stone_slab[type=bottom,waterlogged=false]');
        await send('setblock 426 91 40 stone');
        await send('setblock 426 90 40 vine[up=true]');
        await send('setblock 426 91 40 stone_slab[type=top,waterlogged=false]');

        // Same-face hanging chain: only the upper vine has direct north support.
        await send('setblock 430 91 39 stone');
        await send('setblock 430 91 40 vine[north=true]');
        await send('setblock 430 90 40 vine[north=true]');

        // Mismatched upper face cannot support the lower north face.
        await send('setblock 436 91 40 vine[east=true]');
        await send('setblock 437 91 40 stone');
        await send('setblock 436 90 40 vine[north=true]');

        // Force neighbor updates without becoming permanent supports.
        for (const [x, y, z] of [[430,90,40], [436,90,40]]) {
          await send(`setblock ${x} ${y} ${z - 1} stone`, 80);
          await send(`setblock ${x} ${y} ${z - 1} air`, 80);
        }
        await delay(1000);
        await send('execute unless block 424 90 40 vine run say BOTTOM_SLAB_REJECTED');
        await send('execute if block 426 90 40 vine run say TOP_SLAB_SURVIVES');
        await send('execute if block 430 90 40 vine[north=true] run say HANG_MATCH_SURVIVES');
        await send('execute unless block 436 90 40 vine run say HANG_MISMATCH_GONE');

        const expected = ['BOTTOM_SLAB_REJECTED', 'TOP_SLAB_SURVIVES', 'HANG_MATCH_SURVIVES', 'HANG_MISMATCH_GONE'];
        const found = Object.fromEntries(expected.map(marker => [marker, messages.some(x => x.includes(marker))]));
        const diagnostics = messages.filter(x => /unknown|incorrect|error/i.test(x));
        client.end();
        resolve({name, found, diagnostics});
      } catch (error) { reject(error); }
    });
    client.on('error', reject);
  });
}

Promise.all([run('PUMPKIN', 25565), run('VANILLA', 25575)]).then(results => {
  for (const result of results) console.log(JSON.stringify(result));
  const pass = results.every(result => Object.values(result.found).every(Boolean) && result.diagnostics.length === 0);
  console.log(`VINE_FACE_SUPPORT=${pass ? 'PASS' : 'FAIL'}`);
  if (!pass) process.exitCode = 1;
}).catch(error => { console.error(error); process.exit(1); });

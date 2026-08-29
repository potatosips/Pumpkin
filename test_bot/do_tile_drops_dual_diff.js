const mc = require('minecraft-protocol');
const mcData = require('minecraft-data')('1.21.4');
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));

function flatten(node) {
  if (node == null) return '';
  if (typeof node !== 'object') return String(node);
  if (node.type && node.type !== 'compound' && node.type !== 'list') return flatten(node.value);
  if (node.type === 'list') return (node.value?.value ?? node.value ?? []).map(flatten).join('|');
  return Object.values(node.value ?? node).map(flatten).filter(Boolean).join('|');
}

function run(name, port, x) {
  return new Promise((resolve, reject) => {
    const client = mc.createClient({host: '127.0.0.1', port, username: 'TestBot', version: '1.21.4', auth: 'offline'});
    const passes = [];
    const spawnedItems = {disabled: 0, enabled: 0};
    let phase = 'setup';
    let started = false;
    const send = command => client.write('chat_command', {command, timestamp: BigInt(Date.now())});
    client.on('position', async packet => {
      client.write('teleport_confirm', {teleportId: packet.teleportId});
      if (started) return;
      started = true;
      try {
        await delay(500);
        const selector = `@e[type=item,x=${x - 3},y=65,z=-3,dx=6,dy=10,dz=6]`;
        for (const command of [
          `tp @s ${x} 78 0`, `kill ${selector}`,
          'gamerule doTileDrops false', `setblock ${x} 70 0 stone`,
        ]) { send(command); await delay(350); }
        phase = 'disabled';
        send(`setblock ${x} 70 0 air destroy`);
        await delay(800);
        send(`execute unless entity ${selector} run say PASS_FALSE_NO_BLOCK_DROP`);
        await delay(800);

        send('gamerule doTileDrops true');
        await delay(300);
        send(`setblock ${x} 70 0 stone`);
        await delay(300);
        phase = 'enabled';
        send(`setblock ${x} 70 0 air destroy`);
        await delay(1000);
        send(`execute if entity ${selector} run say PASS_TRUE_BLOCK_DROP`);
        await delay(800);
        send(`kill ${selector}`);
        await delay(200);
        send('gamerule doTileDrops true');
        await delay(200);
        client.end();
        resolve({name, passes, spawnedItems});
      } catch (error) { reject(error); }
    });
    const record = packet => {
      const text = flatten(packet.message ?? packet.content ?? packet);
      if (text.includes('PASS_')) passes.push(text);
    };
    client.on('system_chat', record);
    client.on('profileless_chat', record);
    client.on('disguised_chat', record);
    client.on('player_chat', record);
    client.on('spawn_entity', packet => {
      if (packet.type === mcData.entitiesByName.item.id
          && Math.abs(packet.x - (x + 0.5)) < 4
          && Math.abs(packet.y - 70.5) < 4
          && Math.abs(packet.z - 0.5) < 4
          && spawnedItems[phase] !== undefined) {
        spawnedItems[phase]++;
      }
    });
    client.on('error', reject);
  });
}

Promise.all([run('PUMPKIN', 25565, 490), run('VANILLA', 25575, 510)]).then(results => {
  for (const result of results) console.log(JSON.stringify(result));
  const valid = results.every(result =>
    result.spawnedItems.disabled === 0 && result.spawnedItems.enabled > 0);
  console.log(`DO_TILE_DROPS_BEHAVIOR=${valid ? 'PASS' : 'FAIL'}`);
  if (!valid) process.exitCode = 1;
}).catch(error => { console.error(error); process.exit(1); });

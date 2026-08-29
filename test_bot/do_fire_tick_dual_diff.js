const mc = require('minecraft-protocol');
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
    const changes = {disabled: [], enabled: []};
    const ages = {disabled: [], enabled: []};
    let phase = 'setup';
    let started = false;
    const send = command => client.write('chat_command', {command, timestamp: BigInt(Date.now())});
    client.on('position', async packet => {
      client.write('teleport_confirm', {teleportId: packet.teleportId});
      if (started) return;
      started = true;
      try {
        await delay(500);
        for (const command of [
          `tp @s ${x} 75 0`,
          `fill ${x - 2} 69 -2 ${x + 2} 75 2 air`,
          `setblock ${x} 69 0 netherrack`,
          'gamerule doFireTick false',
          `setblock ${x} 70 0 fire[age=0]`,
        ]) { send(command); await delay(300); }
        await delay(700);
        phase = 'disabled';
        await delay(5000);
        phase = 'disabled_query';
        for (let age = 0; age <= 15; age++) {
          send(`execute if block ${x} 70 0 minecraft:fire[age=${age}] run say FIRE_DISABLED_AGE_${age}`);
          await delay(60);
        }
        await delay(500);

        phase = 'setup';
        send('gamerule doFireTick true');
        await delay(300);
        send(`setblock ${x} 70 0 air`);
        await delay(300);
        send(`setblock ${x} 70 0 fire[age=0]`);
        await delay(700);
        phase = 'enabled';
        await delay(20000);
        phase = 'enabled_query';
        for (let age = 0; age <= 15; age++) {
          send(`execute if block ${x} 70 0 minecraft:fire[age=${age}] run say FIRE_ENABLED_AGE_${age}`);
          await delay(60);
        }
        await delay(500);
        phase = 'setup';
        send('gamerule doFireTick true');
        await delay(300);
        client.end();
        resolve({name, changes, ages});
      } catch (error) { reject(error); }
    });
    client.on('block_change', packet => {
      const {x: bx, y, z} = packet.location;
      if (bx === x && y === 70 && z === 0 && changes[phase]) changes[phase].push(packet.type);
    });
    const recordChat = packet => {
      const text = flatten(packet.message ?? packet.content ?? packet);
      const match = text.match(/FIRE_(DISABLED|ENABLED)_AGE_(\d+)/);
      if (match) ages[match[1].toLowerCase()].push(Number(match[2]));
    };
    client.on('system_chat', recordChat);
    client.on('profileless_chat', recordChat);
    client.on('disguised_chat', recordChat);
    client.on('player_chat', recordChat);
    client.on('error', reject);
  });
}

Promise.all([run('PUMPKIN', 25565, 370), run('VANILLA', 25575, 390)]).then(results => {
  for (const result of results) console.log(JSON.stringify(result));
  const valid = results.every(result => {
    if (result.name === 'VANILLA') {
      return result.ages.disabled.includes(0) && result.ages.enabled.some(age => age !== 0);
    }
    return result.changes.disabled.length === 0 && result.changes.enabled.length > 0;
  });
  console.log(`DO_FIRE_TICK_BEHAVIOR=${valid ? 'PASS' : 'FAIL'}`);
  if (!valid) process.exitCode = 1;
}).catch(error => { console.error(error); process.exit(1); });

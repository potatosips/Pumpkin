const mc = require('minecraft-protocol');

const delay = ms => new Promise(resolve => setTimeout(resolve, ms));

function run(name, port, x) {
  return new Promise((resolve, reject) => {
    const client = mc.createClient({host: '127.0.0.1', port, username: 'TestBot', version: '1.21.4', auth: 'offline'});
    const health = [];
    let started = false;
    let phase = 'join';
    const send = command => client.write('chat_command', {command, timestamp: BigInt(Date.now())});
    const land = async () => {
      for (let y = 119.5; y > 100; y -= 0.5) {
        client.write('position', {x: x + 0.5, y, z: 0.5, flags: {onGround: false, hasHorizontalCollision: false}});
        await delay(50);
      }
      client.write('position', {x: x + 0.5, y: 100, z: 0.5, flags: {onGround: true, hasHorizontalCollision: false}});
    };

    client.on('update_health', packet => health.push({phase, health: packet.health}));
    client.on('position', async packet => {
      client.write('teleport_confirm', {teleportId: packet.teleportId});
      if (started) return;
      started = true;
      try {
        await delay(600);
        for (const command of [
          'gamemode survival',
          'effect clear @s',
          `fill ${x - 3} 99 -3 ${x + 3} 99 3 stone`,
          'gamerule fallDamage false',
          'effect give @s instant_health 1 10 true',
        ]) { send(command); await delay(350); }
        phase = 'disabled';
        send(`tp @s ${x}.5 120 0.5`);
        await delay(500);
        await land();
        await delay(1200);

        phase = 'heal';
        send('effect give @s instant_health 1 10 true');
        await delay(1000);
        send('gamerule fallDamage true');
        await delay(350);
        phase = 'enabled';
        send(`tp @s ${x}.5 120 0.5`);
        await delay(500);
        await land();
        await delay(1200);
        send('gamerule fallDamage true');
        await delay(300);
        client.end();

        const phaseHealth = p => health.filter(h => h.phase === p).map(h => h.health);
        resolve({name, disabled: phaseHealth('disabled'), enabled: phaseHealth('enabled'), all: health});
      } catch (error) { reject(error); }
    });
    client.on('error', reject);
  });
}

Promise.all([run('PUMPKIN', 25565, 310), run('VANILLA', 25575, 330)]).then(results => {
  for (const result of results) console.log(JSON.stringify(result));
  const valid = results.every(result => {
    const disabledMin = Math.min(20, ...result.disabled);
    const enabledMin = Math.min(20, ...result.enabled);
    return disabledMin === 20 && enabledMin < 20;
  });
  console.log(`FALL_DAMAGE_RULE_BEHAVIOR=${valid ? 'PASS' : 'FAIL'}`);
  if (!valid) process.exitCode = 1;
}).catch(error => { console.error(error); process.exit(1); });

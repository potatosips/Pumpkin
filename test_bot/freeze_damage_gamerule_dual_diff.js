const mc = require('minecraft-protocol');
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));

function run(name, port, x) {
  return new Promise((resolve, reject) => {
    const client = mc.createClient({host: '127.0.0.1', port, username: 'TestBot', version: '1.21.4', auth: 'offline'});
    const health = [];
    let phase = 'join';
    let started = false;
    const send = command => client.write('chat_command', {command, timestamp: BigInt(Date.now())});
    const holdPosition = async (px, py, ticks) => {
      for (let i = 0; i < ticks; i++) {
        client.write('position', {x: px, y: py, z: 0.5, flags: {onGround: true, hasHorizontalCollision: false}});
        await delay(50);
      }
    };

    client.on('update_health', packet => health.push({phase, health: packet.health}));
    client.on('position', async packet => {
      client.write('teleport_confirm', {teleportId: packet.teleportId});
      if (started) return;
      started = true;
      try {
        await delay(500);
        for (const command of [
          'gamemode survival', 'effect clear @s', 'gamerule naturalRegeneration false',
          `fill ${x - 2} 69 -2 ${x + 4} 72 2 air`,
          `fill ${x - 2} 69 -2 ${x + 4} 69 2 stone`,
          `setblock ${x} 70 0 powder_snow`,
          'gamerule freezeDamage false', 'effect give @s instant_health 1 10 true',
          `tp @s ${x}.5 70 0.5`,
        ]) { send(command); await delay(300); }
        phase = 'disabled';
        await holdPosition(x + 0.5, 70, 210);

        phase = 'thaw';
        send(`tp @s ${x + 3}.5 70 0.5`);
        await delay(300);
        await holdPosition(x + 3.5, 70, 90);
        send('effect give @s instant_health 1 10 true');
        await delay(700);
        send('gamerule freezeDamage true');
        await delay(300);
        send(`tp @s ${x}.5 70 0.5`);
        await delay(300);
        phase = 'enabled';
        await holdPosition(x + 0.5, 70, 210);
        await delay(1500);

        phase = 'cleanup';
        send('gamerule freezeDamage true');
        await delay(200);
        send('gamerule naturalRegeneration true');
        await delay(200);
        send(`tp @s ${x + 3}.5 70 0.5`);
        await delay(300);
        client.end();
        resolve({name, disabled: health.filter(v => v.phase === 'disabled').map(v => v.health), enabled: health.filter(v => v.phase === 'enabled').map(v => v.health), all: health});
      } catch (error) { reject(error); }
    });
    client.on('error', reject);
  });
}

Promise.all([run('PUMPKIN', 25565, 430), run('VANILLA', 25575, 450)]).then(results => {
  for (const result of results) console.log(JSON.stringify(result));
  const valid = results.every(result =>
    Math.min(20, ...result.disabled) === 20 && Math.min(20, ...result.enabled) < 20);
  console.log(`FREEZE_DAMAGE_RULE_BEHAVIOR=${valid ? 'PASS' : 'FAIL'}`);
  if (!valid) process.exitCode = 1;
}).catch(error => { console.error(error); process.exit(1); });

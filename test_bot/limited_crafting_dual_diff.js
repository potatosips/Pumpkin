const mc = require('minecraft-protocol');

const delay = milliseconds => new Promise(resolve => setTimeout(resolve, milliseconds));

function run(name, port) {
  return new Promise((resolve, reject) => {
    const client = mc.createClient({
      host: '127.0.0.1', port, username: 'TestBot', version: '1.21.4', auth: 'offline'
    });
    const events = [];
    let phase = 'join';
    let loginFlags = null;
    let started = false;

    client.on('packet', (packet, metadata) => {
      if (metadata.name === 'login') {
        loginFlags = {
          reducedDebugInfo: packet.reducedDebugInfo,
          enableRespawnScreen: packet.enableRespawnScreen,
          doLimitedCrafting: packet.doLimitedCrafting
        };
      }
    });
    client.on('game_state_change', packet => {
      if (packet.reason === 12) events.push({phase, reason: packet.reason, value: packet.gameMode});
    });
    client.on('position', async packet => {
      client.write('teleport_confirm', {teleportId: packet.teleportId});
      if (started) return;
      started = true;
      const command = text => client.write('chat_command', {
        command: text, timestamp: BigInt(Date.now())
      });
      try {
        await delay(500);
        phase = 'false';
        command('gamerule doLimitedCrafting false');
        await delay(800);
        phase = 'true';
        command('gamerule doLimitedCrafting true');
        await delay(800);
        phase = 'restore_false';
        command('gamerule doLimitedCrafting false');
        await delay(800);
        phase = 'done';
        client.end();
        resolve({name, loginFlags, events});
      } catch (error) {
        reject(error);
      }
    });
    client.on('error', reject);
  });
}

Promise.all([run('PUMPKIN', 25565), run('VANILLA', 25575)]).then(results => {
  for (const result of results) console.log(JSON.stringify(result));
  const phaseValue = (result, phase, value) => result.events.some(event =>
    event.phase === phase && event.reason === 12 && event.value === value);
  const valid = results.every(result =>
    result.loginFlags !== null
    && result.loginFlags.doLimitedCrafting === false
    && phaseValue(result, 'false', 0)
    && phaseValue(result, 'true', 1)
    && phaseValue(result, 'restore_false', 0));
  console.log(`LIMITED_CRAFTING_BEHAVIOR=${valid ? 'PASS' : 'FAIL'}`);
  if (!valid) process.exitCode = 1;
}).catch(error => {
  console.error(error);
  process.exit(1);
});

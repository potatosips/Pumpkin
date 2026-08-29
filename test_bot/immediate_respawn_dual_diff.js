const mc = require('minecraft-protocol');

const delay = milliseconds => new Promise(resolve => setTimeout(resolve, milliseconds));

function run(name, port) {
  return new Promise((resolve, reject) => {
    const client = mc.createClient({
      host: '127.0.0.1',
      port,
      username: 'TestBot',
      version: '1.21.4',
      auth: 'offline'
    });
    const events = [];
    let phase = 'join';
    let started = false;

    const command = text => client.write('chat_command', {
      command: text,
      timestamp: BigInt(Date.now())
    });

    client.on('game_state_change', packet => {
      if (packet.reason === 11) {
        events.push({phase, reason: packet.reason, value: packet.gameMode});
      }
    });

    client.on('position', async packet => {
      client.write('teleport_confirm', {teleportId: packet.teleportId});
      if (started) return;
      started = true;
      try {
        await delay(500);

        // Both test servers are normally left at the Vanilla default. This
        // same-value assignment also verifies whether callbacks are emitted.
        phase = 'false';
        command('gamerule doImmediateRespawn false');
        await delay(800);

        phase = 'true';
        command('gamerule doImmediateRespawn true');
        await delay(800);

        phase = 'restore_false';
        command('gamerule doImmediateRespawn false');
        await delay(800);

        phase = 'done';
        client.end();
        resolve({name, events});
      } catch (error) {
        reject(error);
      }
    });

    client.on('error', reject);
  });
}

function runTrueJoin(name, port) {
  return new Promise((resolve, reject) => {
    const controller = mc.createClient({
      host: '127.0.0.1', port, username: 'TestBot', version: '1.21.4', auth: 'offline'
    });
    let controllerStarted = false;

    controller.on('position', async packet => {
      controller.write('teleport_confirm', {teleportId: packet.teleportId});
      if (controllerStarted) return;
      controllerStarted = true;
      try {
        await delay(400);
        controller.write('chat_command', {
          command: 'gamerule doImmediateRespawn true',
          timestamp: BigInt(Date.now())
        });
        await delay(700);
        controller.end();
        await delay(500);

        const observer = mc.createClient({
          host: '127.0.0.1', port, username: 'TestBot', version: '1.21.4', auth: 'offline'
        });
        const joinEvents = [];
        let loginFlags = null;
        let observerStarted = false;
        observer.on('packet', (packet, metadata) => {
          if (metadata.name === 'login') {
            loginFlags = {
              reducedDebugInfo: packet.reducedDebugInfo,
              enableRespawnScreen: packet.enableRespawnScreen,
              doLimitedCrafting: packet.doLimitedCrafting
            };
          }
        });
        observer.on('game_state_change', packet => {
          if (packet.reason === 11) joinEvents.push({reason: packet.reason, value: packet.gameMode});
        });
        observer.on('position', async joinPacket => {
          observer.write('teleport_confirm', {teleportId: joinPacket.teleportId});
          if (observerStarted) return;
          observerStarted = true;
          await delay(700);
          observer.write('chat_command', {
            command: 'gamerule doImmediateRespawn false',
            timestamp: BigInt(Date.now())
          });
          await delay(700);
          observer.end();
          resolve({name, loginFlags, joinEvents});
        });
        observer.on('error', reject);
      } catch (error) {
        reject(error);
      }
    });
    controller.on('error', reject);
  });
}

async function main() {
  const results = await Promise.all([
    run('PUMPKIN', 25565),
    run('VANILLA', 25575)
  ]);
  for (const result of results) console.log(JSON.stringify(result));

  const phaseValue = (result, phase, value) => result.events.some(event =>
    event.phase === phase && event.reason === 11 && event.value === value);
  const transitionsValid = results.every(result =>
    phaseValue(result, 'false', 0)
    && phaseValue(result, 'true', 1)
    && phaseValue(result, 'restore_false', 0));

  const trueJoinResults = await Promise.all([
    runTrueJoin('PUMPKIN', 25565),
    runTrueJoin('VANILLA', 25575)
  ]);
  for (const result of trueJoinResults) console.log(JSON.stringify(result));
  const trueJoinValid = trueJoinResults.every(result =>
    result.loginFlags !== null
    && result.loginFlags.enableRespawnScreen === false
    && !result.joinEvents.some(event => event.value === 1));

  const valid = transitionsValid && trueJoinValid;
  console.log(`IMMEDIATE_RESPAWN_BEHAVIOR=${valid ? 'PASS' : 'FAIL'}`);
  if (!valid) process.exitCode = 1;
}

main().catch(error => {
  console.error(error);
  process.exit(1);
});

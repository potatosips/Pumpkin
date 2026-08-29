const mc = require('minecraft-protocol');
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));

function run(name, port) {
  return new Promise((resolve, reject) => {
    const client = mc.createClient({host: '127.0.0.1', port, username: 'TestBot', version: '1.21.4', auth: 'offline'});
    const statuses = [];
    let phase = 'join';
    let started = false;
    const send = command => client.write('chat_command', {command, timestamp: BigInt(Date.now())});

    client.on('entity_status', packet => {
      if ((packet.entityStatus === 22 || packet.entityStatus === 23) && phase !== 'join') {
        statuses.push({phase, entityId: packet.entityId, status: packet.entityStatus});
      }
    });

    client.on('position', async packet => {
      client.write('teleport_confirm', {teleportId: packet.teleportId});
      if (started) return;
      started = true;
      try {
        await delay(500);
        phase = 'enabled';
        send('gamerule reducedDebugInfo true');
        await delay(800);
        phase = 'disabled';
        send('gamerule reducedDebugInfo false');
        await delay(800);
        phase = 'done';
        client.end();
        resolve({name, statuses});
      } catch (error) { reject(error); }
    });
    client.on('error', reject);
  });
}

function runTrueJoin(name, port) {
  return new Promise((resolve, reject) => {
    const controller = mc.createClient({host: '127.0.0.1', port, username: 'TestBot', version: '1.21.4', auth: 'offline'});
    let controllerStarted = false;
    controller.on('position', async packet => {
      controller.write('teleport_confirm', {teleportId: packet.teleportId});
      if (controllerStarted) return;
      controllerStarted = true;
      try {
        await delay(400);
        controller.write('chat_command', {command: 'gamerule reducedDebugInfo true', timestamp: BigInt(Date.now())});
        await delay(700);
        controller.end();
        await delay(500);

        const observer = mc.createClient({host: '127.0.0.1', port, username: 'TestBot', version: '1.21.4', auth: 'offline'});
        let loginReducedDebugInfo = null;
        const joinStatuses = [];
        let observerStarted = false;
        observer.on('packet', (joinPacket, metadata) => {
          if (metadata.name === 'login') loginReducedDebugInfo = joinPacket.reducedDebugInfo;
        });
        observer.on('entity_status', statusPacket => {
          if (statusPacket.entityStatus === 22 || statusPacket.entityStatus === 23) {
            joinStatuses.push(statusPacket.entityStatus);
          }
        });
        observer.on('position', async joinPacket => {
          observer.write('teleport_confirm', {teleportId: joinPacket.teleportId});
          if (observerStarted) return;
          observerStarted = true;
          await delay(700);
          observer.write('chat_command', {command: 'gamerule reducedDebugInfo false', timestamp: BigInt(Date.now())});
          await delay(700);
          observer.end();
          resolve({name, loginReducedDebugInfo, joinStatuses});
        });
        observer.on('error', reject);
      } catch (error) { reject(error); }
    });
    controller.on('error', reject);
  });
}

async function main() {
    const results = await Promise.all([run('PUMPKIN', 25565), run('VANILLA', 25575)]);
    for (const result of results) console.log(JSON.stringify(result));
    const transitionsValid = results.every(result =>
      result.statuses.some(event => event.phase === 'enabled' && event.status === 22)
      && result.statuses.some(event => event.phase === 'disabled' && event.status === 23));
    const joinResults = await Promise.all([runTrueJoin('PUMPKIN', 25565), runTrueJoin('VANILLA', 25575)]);
    for (const result of joinResults) console.log(JSON.stringify(result));
    const joinValid = joinResults.every(result =>
      result.loginReducedDebugInfo === true
      && !result.joinStatuses.includes(22));
    const valid = transitionsValid && joinValid;
    console.log(`REDUCED_DEBUG_INFO_BEHAVIOR=${valid ? 'PASS' : 'FAIL'}`);
    if (!valid) process.exitCode = 1;
}

main().catch(error => { console.error(error); process.exit(1); });

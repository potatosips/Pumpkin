const mc = require('minecraft-protocol');

const delay = milliseconds => new Promise(resolve => setTimeout(resolve, milliseconds));
const ENDER_PEARL_TYPE = require('minecraft-data')('1.21.4').entitiesByName.ender_pearl.id;
function flatten(node) {
  if (node == null) return '';
  if (typeof node !== 'object') return String(node);
  if (node.type && node.type !== 'compound' && node.type !== 'list') return flatten(node.value);
  if (node.type === 'list') return (node.value?.value ?? node.value ?? []).map(flatten).join('|');
  return Object.values(node.value ?? node).map(flatten).filter(Boolean).join('|');
}

function runCase(name, port, vanish) {
  return new Promise((resolve, reject) => {
    const client = mc.createClient({
      host: '127.0.0.1', port, username: 'TestBot', version: '1.21.4', auth: 'offline'
    });
    const observer = mc.createClient({
      host: '127.0.0.1', port, username: 'ObserverBot', version: '1.21.4', auth: 'offline'
    });
    const pearlIds = [];
    const spawnedTypes = [];
    const messages = [];
    const removedIds = new Set();
    let started = false;
    let controllerReady = false;
    let observerReady = false;
    const command = text => client.write('chat_command', {
      command: text, timestamp: BigInt(Date.now())
    });

    observer.on('spawn_entity', packet => {
      spawnedTypes.push(packet.type);
      if (packet.type === ENDER_PEARL_TYPE) pearlIds.push(packet.entityId);
    });
    observer.on('entity_destroy', packet => {
      for (const id of packet.entityIds) removedIds.add(id);
    });
    client.on('system_chat', packet => messages.push(flatten(packet.content)));

    async function startWhenReady() {
      if (started || !controllerReady || !observerReady) return;
      started = true;
      try {
        client.write('client_command', {actionId: 0});
        observer.write('client_command', {actionId: 0});
        await delay(600);
        // minecraft-data's 1.21.4 schema omits this empty serverbound packet,
        // so send its verified protocol ID directly.
        client.writeRaw(Buffer.from([0x2a]));
        observer.writeRaw(Buffer.from([0x2a]));
        await delay(1000);
        command('kill @e[type=ender_pearl]');
        await delay(300);
        command(`gamerule enderPearlsVanishOnDeath ${vanish}`);
        await delay(300);
        command('tp @s 620 150 0 0 -90');
        command('tp ObserverBot 620 150 2 0 0');
        await delay(1000);
        command('item replace entity @s weapon.mainhand with ender_pearl');
        await delay(400);
        client.write('held_item_slot', {slotId: 0});
        for (let attempt = 1; attempt <= 10 && pearlIds.length === 0; attempt++) {
          client.write('use_item', {hand: 0, sequence: attempt, rotation: {x: 0, y: -90}});
          await delay(500);
        }
        if (pearlIds.length > 0) {
          command('data merge entity @e[type=ender_pearl,limit=1,sort=nearest] {NoGravity:1b}');
          await delay(300);
        }

        const thrownId = pearlIds[pearlIds.length - 1] ?? null;
        command('kill @s');
        await delay(900);
        const removedAfterDeath = thrownId !== null && removedIds.has(thrownId);
        client.end();
        observer.end();
        resolve({
          name,
          vanish,
          thrownId,
          removedAfterDeath,
          itemCommandSucceeded: messages.some(message => message.includes('commands.item.entity.set.success.single')),
          sawPearlType: spawnedTypes.includes(ENDER_PEARL_TYPE)
        });
      } catch (error) { reject(error); }
    }

    client.on('position', packet => {
      client.write('teleport_confirm', {teleportId: packet.teleportId});
      controllerReady = true;
      startWhenReady();
    });
    observer.on('position', packet => {
      observer.write('teleport_confirm', {teleportId: packet.teleportId});
      observerReady = true;
      startWhenReady();
    });
    client.on('error', reject);
    observer.on('error', reject);
  });
}

function queryPresence(port) {
  return new Promise((resolve, reject) => {
    const client = mc.createClient({host: '127.0.0.1', port, username: 'TestBot', version: '1.21.4', auth: 'offline'});
    let present = false;
    let started = false;
    client.on('system_chat', packet => {
      if (flatten(packet.content).includes('PEARL_PRESENT')) present = true;
    });
    client.on('position', async packet => {
      client.write('teleport_confirm', {teleportId: packet.teleportId});
      if (started) return;
      started = true;
      client.write('client_command', {actionId: 0});
      await delay(600);
      await delay(3500);
      client.write('chat_command', {
        command: 'execute if entity @e[type=ender_pearl] run say PEARL_PRESENT',
        timestamp: BigInt(Date.now())
      });
      await delay(600);
      client.write('chat_command', {command: 'kill @e[type=ender_pearl]', timestamp: BigInt(Date.now())});
      await delay(400);
      client.end();
      resolve(present);
    });
    client.on('error', reject);
  });
}

async function runServer(name, port) {
  const disabled = await runCase(name, port, false);
  await delay(700);
  disabled.presentAfterReconnect = await queryPresence(port);
  await delay(700);
  const enabled = await runCase(name, port, true);
  await delay(700);
  enabled.presentAfterReconnect = await queryPresence(port);
  return {name, disabled, enabled};
}

Promise.all([runServer('PUMPKIN', 25565), runServer('VANILLA', 25575)]).then(results => {
  for (const result of results) console.log(JSON.stringify(result));
  const valid = results.every(result =>
    result.disabled.thrownId !== null
    && result.enabled.thrownId !== null
    && result.disabled.removedAfterDeath === false
    && result.enabled.removedAfterDeath === true);
  console.log(`ENDER_PEARLS_VANISH_ON_DEATH_BEHAVIOR=${valid ? 'PASS' : 'FAIL'}`);
  if (!valid) process.exitCode = 1;
}).catch(error => {
  console.error(error);
  process.exit(1);
});

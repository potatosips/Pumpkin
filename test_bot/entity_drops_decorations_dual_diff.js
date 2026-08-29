const mc = require('minecraft-protocol');

const delay = ms => new Promise(resolve => setTimeout(resolve, ms));

function translationKey(node) {
  if (!node || typeof node !== 'object') return null;
  if (node.type === 'compound') return translationKey(node.value);
  if (node.translate?.value) return node.translate.value;
  for (const value of Object.values(node.value ?? node)) {
    const found = translationKey(value);
    if (found) return found;
  }
  return null;
}

function normalizeDynamicEntityUuid(node, key = '') {
  if (Array.isArray(node)) return node.map(value => normalizeDynamicEntityUuid(value));
  if (!node || typeof node !== 'object') return node;
  if (key === 'insertion' && node.type === 'string'
      && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(node.value)) {
    return {type: 'string', value: '<entity-uuid>'};
  }
  if (key === 'id' && node.type === 'intArray' && Array.isArray(node.value) && node.value.length === 4) {
    return {type: 'intArray', value: ['<entity-uuid>']};
  }
  return Object.fromEntries(Object.keys(node).sort().map(childKey => [
    childKey,
    normalizeDynamicEntityUuid(node[childKey], childKey),
  ]));
}

function run(name, port, x) {
  return new Promise((resolve, reject) => {
    const client = mc.createClient({host: '127.0.0.1', port, username: 'TestBot', version: '1.21.4', auth: 'offline'});
    const messages = [];
    let started = false;
    const send = command => client.write('chat_command', {command, timestamp: BigInt(Date.now())});

    client.on('system_chat', packet => messages.push(packet.content));
    client.on('position', async packet => {
      client.write('teleport_confirm', {teleportId: packet.teleportId});
      if (started) return;
      started = true;
      try {
        await delay(500);
        const observations = [];
        for (const scenario of [
          {enabled: false, gamemode: 'survival'},
          {enabled: true, gamemode: 'survival'},
          {enabled: true, gamemode: 'creative'},
        ]) {
          for (const command of [
            `gamemode ${scenario.gamemode}`,
            `tp @s ${x}.5 100 0.5`,
            `kill @e[type=minecraft:item,distance=..16]`,
            `kill @e[type=minecraft:item_frame,distance=..16]`,
            `gamerule doEntityDrops ${scenario.enabled}`,
            `setblock ${x} 100 1 stone`,
            `summon minecraft:item_frame ${x}.5 100.5 0.96875 {Facing:2b,Item:{id:"minecraft:diamond",count:1}}`,
            `damage @e[type=minecraft:item_frame,distance=..4,limit=1,sort=nearest] 100 minecraft:arrow by @s from @s`,
            `damage @e[type=minecraft:item_frame,distance=..4,limit=1,sort=nearest] 100 minecraft:arrow by @s from @s`,
          ]) { send(command); await delay(300); }
          const before = messages.length;
          send(`kill @e[type=minecraft:item,distance=..16]`);
          await delay(500);
          observations.push({...scenario, response: messages.slice(before)});
        }
        for (const command of [
          `kill @e[type=minecraft:item,distance=..16]`,
          `summon minecraft:item ${x}.5 100 8.5 {Item:{id:"minecraft:item_frame",count:1}}`,
        ]) { send(command); await delay(300); }
        const feedbackBefore = messages.length;
        send(`kill @e[type=minecraft:item,distance=..16,limit=1,sort=nearest]`);
        await delay(500);
        const itemFrameFeedback = messages.slice(feedbackBefore);
        send('gamerule doEntityDrops true');
        await delay(200);
        client.end();
        resolve({name, observations, itemFrameFeedback});
      } catch (error) { reject(error); }
    });
    client.on('error', reject);
  });
}

Promise.all([run('PUMPKIN', 25565, 720), run('VANILLA', 25575, 740)])
  .then(results => {
    for (const result of results) console.log(JSON.stringify(result));
    const pass = results.every(result => {
      const key = (enabled, gamemode) => translationKey(
        result.observations.find(entry => entry.enabled === enabled && entry.gamemode === gamemode).response[0]
      );
      const presentKey = key(true, 'survival');
      return key(false, 'survival') === 'argument.entity.notfound.entity'
        && (presentKey === 'commands.kill.success.single' || presentKey === 'commands.kill.success.multiple')
        && key(true, 'creative') === 'argument.entity.notfound.entity';
    });
    const exactWindows = result => [
      result.observations[0].response,
      result.itemFrameFeedback,
      result.observations[2].response,
    ];
    const exact = JSON.stringify(normalizeDynamicEntityUuid(exactWindows(results[0])))
      === JSON.stringify(normalizeDynamicEntityUuid(exactWindows(results[1])));
    console.log(`ENTITY_DROPS_ITEM_FRAME=${pass ? 'PASS' : 'FAIL'}`);
    console.log(`ENTITY_DROPS_FEEDBACK_PACKET_WINDOWS=${exact ? '3/3' : 'FAIL'}`);
    if (!pass || !exact) process.exitCode = 1;
  })
  .catch(error => { console.error(error); process.exit(1); });

const mc = require('minecraft-protocol');

const delay = ms => new Promise(resolve => setTimeout(resolve, ms));

function normalize(node, key = '') {
  if (Array.isArray(node)) return node.map(value => normalize(value));
  if (!node || typeof node !== 'object') return node;
  if (key === 'insertion' && node.type === 'string'
      && /^[0-9a-f-]{36}$/i.test(node.value)) {
    return {type: 'string', value: '<entity-uuid>'};
  }
  if (key === 'id' && node.type === 'intArray' && node.value?.length === 4) {
    return {type: 'intArray', value: ['<entity-uuid>']};
  }
  return Object.fromEntries(Object.keys(node).sort().map(childKey => [
    childKey,
    normalize(node[childKey], childKey),
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
        client.write('client_command', {actionId: 0});
        await delay(500);
        client.writeRaw(Buffer.from([0x2a]));
        await delay(500);
        send(`tp @s ${x}.5 100 0.5`);
        await delay(300);
        const scenarios = [
          {
            name: 'default',
            nbt: '{Item:{id:"minecraft:item_frame",count:1}}',
          },
          {
            name: 'custom_name',
            itemCommand: `give @s minecraft:diamond[custom_name='{"text":"Parity Gem","color":"gold","italic":false}'] 1`,
          },
          {
            name: 'item_name',
            nbt: `{Item:{id:"minecraft:diamond",count:1,components:{"minecraft:item_name":'{"text":"Artifact","color":"aqua"}'}}}`,
          },
        ];
        const observations = [];
        for (const scenario of scenarios) {
          send(`kill @e[type=minecraft:item,distance=..16]`);
          await delay(250);
          if (scenario.itemCommand) {
            send(`summon minecraft:item ${x}.5 100 8.5 {Item:{id:"minecraft:diamond",count:1}}`);
            await delay(250);
            const itemCommandBefore = messages.length;
            send(`item replace entity @e[type=minecraft:item,distance=..16,limit=1,sort=nearest] contents with minecraft:diamond[custom_name='{"text":"Parity Gem","color":"gold","italic":false}']`);
            await delay(250);
            scenario.itemCommandResponse = messages.slice(itemCommandBefore);
          } else {
            send(`summon minecraft:item ${x}.5 100 8.5 ${scenario.nbt}`);
          }
          await delay(250);
          const dataBefore = messages.length;
          send('data get entity @e[type=minecraft:item,distance=..16,limit=1,sort=nearest] Item.components');
          await delay(250);
          const storedComponents = messages.slice(dataBefore);
          const before = messages.length;
          send('kill @e[type=minecraft:item,distance=..16,limit=1,sort=nearest]');
          await delay(400);
          observations.push({name: scenario.name, itemCommandResponse: scenario.itemCommandResponse, storedComponents, response: messages.slice(before)});
        }
        client.end();
        resolve({name, observations});
      } catch (error) { reject(error); }
    });
    client.on('error', reject);
  });
}

Promise.all([run('PUMPKIN', 25565, 760), run('VANILLA', 25575, 780)])
  .then(results => {
    for (const result of results) console.log(JSON.stringify(result));
    let matched = 0;
    for (let i = 0; i < results[0].observations.length; i++) {
      const pumpkin = JSON.stringify(normalize(results[0].observations[i].response));
      const vanilla = JSON.stringify(normalize(results[1].observations[i].response));
      const isSuccess = JSON.stringify(results[0].observations[i].response).includes('commands.kill.success.single')
        && JSON.stringify(results[1].observations[i].response).includes('commands.kill.success.single');
      if (pumpkin === vanilla && isSuccess) matched++;
      else console.log(`MISMATCH ${results[0].observations[i].name}`);
    }
    console.log(`ITEM_ENTITY_NAME_PACKET_WINDOWS=${matched}/${results[0].observations.length}`);
    const pumpkinCustomSetup = JSON.stringify(results[0].observations[1].itemCommandResponse ?? []);
    const vanillaCustomSetup = JSON.stringify(results[1].observations[1].itemCommandResponse ?? []);
    const customSlotPass = pumpkinCustomSetup.includes('commands.item.entity.set.success.single')
      && vanillaCustomSetup.includes('commands.item.entity.set.success.single');
    const customSlotGap = pumpkinCustomSetup.includes('commands.item.target.no_such_slot')
      && vanillaCustomSetup.includes('commands.item.entity.set.success.single');
    console.log(`ITEM_ENTITY_CONTENTS_SLOT=${customSlotPass ? 'PASS' : customSlotGap ? 'KNOWN_GAP' : 'RECHECK'}`);
    if (matched !== results[0].observations.length || !customSlotPass) process.exitCode = 1;
  })
  .catch(error => { console.error(error); process.exit(1); });

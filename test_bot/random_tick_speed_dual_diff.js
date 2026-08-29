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
    let started = false;
    let phase = 'setup';
    const updates = {zero: 0, high: 0};
    const highLocations = new Set();
    const passes = [];
    const timeQueries = [];
    const growthResponses = [];
    const growthCounts = [];
    let queryingGrowth = false;
    const send = command => client.write('chat_command', {command, timestamp: BigInt(Date.now())});
    const command = async (text, wait = 300) => { send(text); await delay(wait); };

    function inCropArea(location) {
      const px = location?.x ?? location?.[0];
      const py = location?.y ?? location?.[1];
      const pz = location?.z ?? location?.[2];
      return py === 70 && px >= x && px <= x + 15 && pz >= 0 && pz <= 15;
    }

    function recordCropUpdate(location) {
      if ((phase === 'zero' || phase === 'high') && inCropArea(location)) {
        updates[phase]++;
        if (phase === 'high') highLocations.add(`${location.x},${location.y},${location.z}`);
      }
    }

    client.on('block_change', packet => recordCropUpdate(packet.location));
    client.on('multi_block_change', packet => {
      const section = packet.chunkCoordinates;
      for (const rawRecord of packet.records ?? []) {
        const record = BigInt(rawRecord);
        const local = Number(record & 0xfffn);
        recordCropUpdate({
          x: section.x * 16 + ((local >> 8) & 15),
          y: section.y * 16 + (local & 15),
          z: section.z * 16 + ((local >> 4) & 15),
        });
      }
    });

    client.on('position', async packet => {
      client.write('teleport_confirm', {teleportId: packet.teleportId});
      if (started) return;
      started = true;
      try {
        await delay(500);
        await command(`tp @s ${x + 8} 76 8`);
        await command('time set day');
        await command('gamerule randomTickSpeed 0');
        await command(`fill ${x} 68 0 ${x + 31} 71 15 air`);
        await command(`fill ${x} 69 0 ${x + 31} 69 15 farmland[moisture=7]`);
        await command(`fill ${x} 70 0 ${x + 31} 70 15 wheat[age=0]`, 800);

        phase = 'zero';
        await delay(4000);
        phase = 'setup';
        await command(`execute if blocks ${x} 70 0 ${x + 15} 70 15 ${x + 16} 70 0 all run say PASS_ZERO_FIELD_UNCHANGED`, 500);

        await command(`fill ${x} 70 0 ${x + 31} 70 15 wheat[age=0]`, 800);
        await command('gamerule randomTickSpeed 100');
        await command('time query gametime', 500);
        phase = 'high';
        await delay(15000);
        phase = 'setup';
        await command('time query gametime', 500);
        queryingGrowth = true;
        for (let age = 1; age <= 7; age++) {
          await command(`fill ${x} 70 0 ${x + 15} 70 15 wheat[age=0] replace wheat[age=${age}]`, 500);
        }
        queryingGrowth = false;

        await command('gamerule randomTickSpeed 3');
        await command(`fill ${x} 68 0 ${x + 31} 71 15 air`);
        client.end();
        resolve({name, updates, uniqueHighLocations: highLocations.size, highLocations: [...highLocations].sort(), timeQueries, growthCounts, grownBlocks: growthCounts.reduce((sum, count) => sum + count, 0), growthResponses, passes});
      } catch (error) { reject(error); }
    });

    const record = packet => {
      const text = flatten(packet.message ?? packet.content ?? packet);
      if (text.includes('commands.time.query')) timeQueries.push(text);
      if (queryingGrowth && text) {
        growthResponses.push(text);
        if (text.includes('commands.fill.success')) {
          const count = Number(text.match(/\d+/)?.[0]);
          if (Number.isFinite(count)) growthCounts.push(count);
        }
      }
      if (text.includes('PASS_') && !text.includes('command.context.here')) passes.push(text);
    };
    client.on('system_chat', record);
    client.on('profileless_chat', record);
    client.on('disguised_chat', record);
    client.on('player_chat', record);
    client.on('error', reject);
  });
}

Promise.all([run('PUMPKIN', 25565, 600), run('VANILLA', 25575, 650)])
  .then(results => {
    for (const result of results) console.log(JSON.stringify(result));
    const valid = results.every(result =>
      result.updates.zero === 0
      && result.grownBlocks > 0);
    console.log(`RANDOM_TICK_SPEED_BEHAVIOR=${valid ? 'PASS' : 'FAIL'}`);
    if (!valid) process.exitCode = 1;
  })
  .catch(error => { console.error(error); process.exit(1); });

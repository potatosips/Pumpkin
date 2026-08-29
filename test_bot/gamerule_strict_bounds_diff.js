const mc = require('minecraft-protocol');

// Each case owns a fixed response window. Brigadier errors can emit multiple
// chat packets, so advancing on the first packet corrupts later comparisons.
const cases = [
  ['gamerule spawnChunkRadius -1', 'reject'],
  ['gamerule spawnChunkRadius 33', 'reject'],
  ['gamerule spawnChunkRadius 0', 'accept'],
  ['gamerule spawnChunkRadius', 'query'],
  ['gamerule spawnChunkRadius 32', 'accept'],
  ['gamerule spawnChunkRadius 2', 'restore'],
  ['gamerule snowAccumulationHeight -1', 'accept'],
  ['gamerule snowAccumulationHeight 2147483647', 'accept'],
  ['gamerule snowAccumulationHeight -2147483648', 'accept'],
  ['gamerule snowAccumulationHeight 2147483648', 'reject'],
  ['gamerule snowAccumulationHeight 1', 'restore'],
  ['gamerule randomTickSpeed -1', 'accept'],
  ['gamerule randomTickSpeed 3', 'restore'],
  ['gamerule spawnRadius -1', 'accept'],
  ['gamerule spawnRadius 10', 'restore'],
  ['gamerule doDaylightCycle 1', 'reject'],
  ['gamerule disableRaids true', 'accept'],
  ['gamerule disableRaids', 'query'],
  ['gamerule disableRaids false', 'restore'],
  ['gamerule doFireTick false', 'accept'],
  ['gamerule doFireTick', 'query'],
  ['gamerule doFireTick true', 'restore'],
];

function canonical(value) {
  if (value === undefined) return null;
  if (typeof value === 'bigint') return value.toString();
  if (Array.isArray(value)) return value.map(canonical);
  if (value && typeof value === 'object') {
    return Object.fromEntries(Object.keys(value).sort().map(k => [k, canonical(value[k])]));
  }
  return value;
}

function run(port) {
  return new Promise((resolve, reject) => {
    const client = mc.createClient({host: '127.0.0.1', port, username: 'TestBot', version: '1.21.4', auth: 'offline'});
    const results = [];
    let active = null;
    let started = false;
    client.on('system_chat', packet => { if (active) active.push(canonical(packet)); });
    client.on('position', async () => {
      if (started) return;
      started = true;
      try {
        await new Promise(r => setTimeout(r, 500));
        for (const [command] of cases) {
          const packets = [];
          active = packets;
          client.write('chat_command', {command, timestamp: BigInt(Date.now())});
          await new Promise(r => setTimeout(r, 700));
          active = null;
          results.push(packets);
        }
        client.end();
        resolve(results);
      } catch (error) { reject(error); }
    });
    client.on('error', reject);
  });
}

Promise.all([run(25565), run(25575)]).then(([pumpkin, vanilla]) => {
  let matches = 0;
  cases.forEach(([command, purpose], index) => {
    const p = JSON.stringify(pumpkin[index]);
    const v = JSON.stringify(vanilla[index]);
    const matched = p === v;
    if (matched) matches++;
    console.log(`${matched ? 'MATCH' : 'DIFF '} [${purpose}] ${command}`);
    if (!matched) console.log(`  P=${p}\n  V=${v}`);
  });
  console.log(`EXACT_PACKET_WINDOWS=${matches}/${cases.length}`);
  if (matches !== cases.length) process.exitCode = 1;
}).catch(error => { console.error(error); process.exit(1); });

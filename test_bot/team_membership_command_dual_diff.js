const mc = require('minecraft-protocol');

const delay = ms => new Promise(resolve => setTimeout(resolve, ms));

const cases = [
  'team remove parity_member_a',
  'team remove parity_member_b',
  'team add parity_member_a',
  'team add parity_member_b',
  'team join parity_member_a TestBot',
  'team join parity_member_a TestBot',
  'team list parity_member_a',
  'team join parity_member_b TestBot',
  'team list parity_member_a',
  'team list parity_member_b',
  'team leave TestBot',
  'team list parity_member_b',
  'team leave TestBot',
  'team empty parity_member_a',
  'team remove parity_member_a',
  'team remove parity_member_b',
];

const expectedKeys = [
  'team.notFound',
  'team.notFound',
  'commands.team.add.success',
  'commands.team.add.success',
  'commands.team.join.success.single',
  'commands.team.join.success.single',
  'commands.team.list.members.success',
  'commands.team.join.success.single',
  'commands.team.list.members.empty',
  'commands.team.list.members.success',
  'commands.team.leave.success.single',
  'commands.team.list.members.empty',
  'commands.team.leave.success.single',
  'commands.team.empty.unchanged',
  'commands.team.remove.success',
  'commands.team.remove.success',
];

function canonical(value) {
  if (value === undefined) return null;
  if (typeof value === 'bigint') return value.toString();
  if (Array.isArray(value)) return value.map(canonical);
  if (value && typeof value === 'object') {
    return Object.fromEntries(Object.keys(value).sort().map(key => [key, canonical(value[key])]));
  }
  return value;
}

function translationRecords(value, records = []) {
  if (!value || typeof value !== 'object') return records;
  if (value.translate && value.translate.type === 'string') {
    const withTag = value.with;
    const args = withTag && withTag.value && Array.isArray(withTag.value.value)
      ? withTag.value.value.length
      : 0;
    records.push({key: value.translate.value, args});
  }
  for (const child of Object.values(value)) translationRecords(child, records);
  return records;
}

function primaryTranslation(packets) {
  const records = translationRecords(packets);
  return records.find(record => record.key.startsWith('commands.team.') || record.key === 'team.notFound') || null;
}

function run(name, port) {
  return new Promise((resolve, reject) => {
    const client = mc.createClient({host: '127.0.0.1', port, username: 'TestBot', version: '1.21.4', auth: 'offline'});
    const results = [];
    let active = null;
    let started = false;
    client.on('system_chat', packet => { if (active) active.push(canonical(packet)); });
    client.on('position', async packet => {
      client.write('teleport_confirm', {teleportId: packet.teleportId});
      if (started) return;
      started = true;
      client.write('client_command', {actionId: 0});
      setTimeout(() => client.writeRaw(Buffer.from([0x2a])), 150);
      try {
        await delay(500);
        for (const command of cases) {
          const packets = [];
          active = packets;
          client.write('chat_command', {command, timestamp: BigInt(Date.now())});
          await delay(500);
          active = null;
          results.push(packets);
        }
        client.end();
        resolve({name, results});
      } catch (error) { reject(error); }
    });
    client.on('error', reject);
  });
}

Promise.all([run('PUMPKIN', 25565), run('VANILLA', 25575)]).then(([pumpkin, vanilla]) => {
  let matches = 0;
  let semanticPass = true;
  cases.forEach((command, index) => {
    const p = JSON.stringify(pumpkin.results[index]);
    const v = JSON.stringify(vanilla.results[index]);
    const matched = p === v;
    if (matched) matches++;
    console.log(`${matched ? 'MATCH' : 'DIFF '} ${command}`);
    if (!matched) console.log(`  P=${p}\n  V=${v}`);
    const pTranslation = primaryTranslation(pumpkin.results[index]);
    const vTranslation = primaryTranslation(vanilla.results[index]);
    const expected = expectedKeys[index];
    const semanticMatch = pTranslation?.key === expected && vTranslation?.key === expected;
    if (!semanticMatch) semanticPass = false;
    console.log(`  SEMANTIC=${semanticMatch ? 'MATCH' : 'FAIL'} key=${expected} P=${JSON.stringify(pTranslation)} V=${JSON.stringify(vTranslation)}`);
  });
  const emptyIndex = cases.indexOf('team empty parity_member_a');
  const pEmpty = primaryTranslation(pumpkin.results[emptyIndex]);
  const vEmpty = primaryTranslation(vanilla.results[emptyIndex]);
  const emptyArityMatch = pEmpty?.args === 0 && vEmpty?.args === 0;
  if (!emptyArityMatch) semanticPass = false;
  console.log(`TEAM_MEMBERSHIP_PACKET_WINDOWS=${matches}/${cases.length}`);
  console.log(`TEAM_EMPTY_UNCHANGED_ARITY=${emptyArityMatch ? 'PASS' : 'FAIL'}`);
  console.log(`TEAM_MEMBERSHIP_SEMANTICS=${semanticPass ? 'PASS' : 'FAIL'}`);
  if (!semanticPass) process.exitCode = 1;
}).catch(error => { console.error(error); process.exit(1); });

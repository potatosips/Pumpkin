const mc = require('minecraft-protocol');

const delay = ms => new Promise(resolve => setTimeout(resolve, ms));

function flatten(node) {
  if (node == null) return '';
  if (typeof node !== 'object') return String(node);
  if (node.type && node.type !== 'compound' && node.type !== 'list') return flatten(node.value);
  if (node.type === 'list') return (node.value?.value ?? node.value ?? []).map(flatten).join('|');
  return Object.values(node.value ?? node).map(flatten).filter(Boolean).join('|');
}

function connect(username, port) {
  return new Promise((resolve, reject) => {
    const client = mc.createClient({host: '127.0.0.1', port, username, version: '1.21.4', auth: 'offline'});
    let joined = false;
    client.on('position', packet => {
      client.write('teleport_confirm', {teleportId: packet.teleportId});
      if (!joined) {
        joined = true;
        client.write('client_command', {actionId: 0});
        setTimeout(() => client.writeRaw(Buffer.from([0x2a])), 150);
        resolve(client);
      }
    });
    client.on('error', reject);
  });
}

function collectMessages(client, messages) {
  const record = packet => messages.push(flatten(packet.message ?? packet.content ?? packet));
  for (const event of ['system_chat', 'profileless_chat', 'disguised_chat', 'player_chat']) {
    client.on(event, record);
  }
}

async function run(name, port) {
  const admin = await connect('TestBot', port);
  const teammateName = `DTeam${port}`;
  const outsiderName = `DOut${port}`;
  const teammate = await connect(teammateName, port);
  const outsider = await connect(outsiderName, port);
  const teammateMessages = [];
  const outsiderMessages = [];
  collectMessages(teammate, teammateMessages);
  collectMessages(outsider, outsiderMessages);

  const command = async (text, wait = 260) => {
    admin.write('chat_command', {command: text, timestamp: BigInt(Date.now())});
    await delay(wait);
  };

  await command('gamerule showDeathMessages true');
  await command(`team leave ${outsiderName}`);

  const modes = [
    ['always', true, true],
    ['never', false, false],
    ['hideForOtherTeams', true, false],
    ['hideForOwnTeam', false, true],
  ];
  const observations = [];

  for (let index = 0; index < modes.length; index++) {
    const [mode, expectedTeammate, expectedOutsider] = modes[index];
    const teamName = `deathvis${index}`;
    const victimName = `DV${index}${port}${Date.now() % 100000}`;
    await command(`team remove ${teamName}`);
    await command(`team add ${teamName}`);
    await command(`team join ${teamName} ${teammateName}`);
    const victim = await connect(victimName, port);
    await command(`team join ${teamName} ${victimName}`);
    await command(`team modify ${teamName} deathMessageVisibility ${mode}`);
    teammateMessages.length = 0;
    outsiderMessages.length = 0;
    await command(`kill ${victimName}`);
    await delay(500);
    const isDeathMessage = message => message.includes(victimName) && message.includes('death.attack.');
    const teammateSaw = teammateMessages.some(isDeathMessage);
    const outsiderSaw = outsiderMessages.some(isDeathMessage);
    observations.push({
      mode,
      teammateSaw,
      outsiderSaw,
      expectedTeammate,
      expectedOutsider,
    });
    victim.end();
    await command(`team remove ${teamName}`);
  }

  const mutedVictimName = `DVM${port}${Date.now() % 100000}`;
  const mutedVictim = await connect(mutedVictimName, port);
  await command('gamerule showDeathMessages false');
  teammateMessages.length = 0;
  outsiderMessages.length = 0;
  await command(`kill ${mutedVictimName}`);
  await delay(500);
  const isMutedDeathMessage = message =>
    message.includes(mutedVictimName) && message.includes('death.attack.');
  observations.push({
    mode: 'globalFalse',
    teammateSaw: teammateMessages.some(isMutedDeathMessage),
    outsiderSaw: outsiderMessages.some(isMutedDeathMessage),
    expectedTeammate: false,
    expectedOutsider: false,
  });
  mutedVictim.end();
  await command('gamerule showDeathMessages true');
  for (const client of [admin, teammate, outsider]) client.end();
  const pass = observations.every(result =>
    result.teammateSaw === result.expectedTeammate
      && result.outsiderSaw === result.expectedOutsider);
  return {name, pass, observations};
}

Promise.all([run('PUMPKIN', 25565), run('VANILLA', 25575)]).then(results => {
  for (const result of results) console.log(JSON.stringify(result));
  const pass = results.every(result => result.pass);
  console.log(`DEATH_MESSAGE_VISIBILITY=${pass ? 'PASS' : 'FAIL'}`);
  if (!pass) process.exitCode = 1;
}).catch(error => { console.error(error); process.exit(1); });

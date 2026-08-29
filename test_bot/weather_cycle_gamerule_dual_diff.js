const mc = require('minecraft-protocol');

const delay = milliseconds => new Promise(resolve => setTimeout(resolve, milliseconds));

function flatten(node) {
  if (node == null) return '';
  if (typeof node !== 'object') return String(node);
  if (node.type && node.type !== 'compound' && node.type !== 'list') return flatten(node.value);
  if (node.type === 'list') return (node.value?.value ?? node.value ?? []).map(flatten).join('|');
  return Object.values(node.value ?? node).map(flatten).filter(Boolean).join('|');
}

function run(name, port) {
  return new Promise((resolve, reject) => {
    const client = mc.createClient({host: '127.0.0.1', port, username: 'TestBot', version: '1.21.4', auth: 'offline'});
    const events = [];
    const diagnostics = [];
    let phase = 'join';
    let started = false;
    const command = text => client.write('chat_command', {command: text, timestamp: BigInt(Date.now())});
    const send = async (text, wait = 300) => { command(text); await delay(wait); };
    const record = packet => {
      const value = flatten(packet.message ?? packet.content ?? packet);
      if (/unknown|incorrect|error/i.test(value)) diagnostics.push(value);
    };
    client.on('system_chat', record);
    client.on('profileless_chat', record);
    client.on('disguised_chat', record);
    client.on('player_chat', record);
    client.on('game_state_change', packet => {
      if (packet.reason === 1 || packet.reason === 2 || packet.reason === 7) {
        events.push({phase, reason: packet.reason, value: packet.gameMode});
      }
    });
    client.on('position', async packet => {
      client.write('teleport_confirm', {teleportId: packet.teleportId});
      if (started) return;
      started = true;
      try {
        client.write('client_command', {actionId: 0});
        await delay(600);
        client.writeRaw(Buffer.from([0x2a]));
        await delay(800);
        await send('gamemode spectator @s');

        phase = 'setup';
        await send('gamerule doWeatherCycle true');
        await send('weather clear 20t', 600);
        await send('gamerule doWeatherCycle false');
        await send('weather rain 20t', 300);

        phase = 'frozen';
        await delay(2500);

        phase = 'advancing';
        await send('gamerule doWeatherCycle true', 2500);

        phase = 'cleanup';
        await send('weather clear');
        await send('gamerule doWeatherCycle true');

        const frozenEnd = events.filter(event => event.phase === 'frozen' && event.reason === 2).length;
        const frozenLevels = events.filter(event => event.phase === 'frozen' && event.reason === 7).map(event => event.value);
        const frozenRainLevelChanges = frozenLevels.length;
        const advancingEnd = events.filter(event => event.phase === 'advancing' && event.reason === 2).length;
        const advancingLevels = events.filter(event => event.phase === 'advancing' && event.reason === 7).map(event => event.value);
        const frozenOnlyRises = frozenLevels.every((value, index) => index === 0 || value >= frozenLevels[index - 1]);
        const advancingReverses = advancingLevels.some((value, index) => index > 0 && value < advancingLevels[index - 1]);
        client.end();
        resolve({
          name,
          frozenEnd,
          frozenRainLevelChanges,
          frozenOnlyRises,
          advancingEnd,
          advancingReverses,
          diagnostics
        });
      } catch (error) { reject(error); }
    });
    client.on('error', reject);
  });
}

Promise.all([run('PUMPKIN', 25565), run('VANILLA', 25575)]).then(results => {
  for (const result of results) console.log(JSON.stringify(result));
  const pass = results.every(result =>
    result.diagnostics.length === 0 &&
    result.frozenEnd === 0 &&
    result.frozenRainLevelChanges > 0 &&
    result.frozenOnlyRises &&
    result.advancingReverses
  );
  console.log(`WEATHER_CYCLE_GAMERULE=${pass ? 'PASS' : 'FAIL'}`);
  if (!pass) process.exitCode = 1;
}).catch(error => {
  console.error(error);
  process.exit(1);
});

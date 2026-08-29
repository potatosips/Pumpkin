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
    const client = mc.createClient({
      host: '127.0.0.1', port, username: 'TestBot', version: '1.21.4', auth: 'offline'
    });
    const passes = new Set();
    const messages = [];
    let started = false;
    const send = command => client.write('chat_command', {command, timestamp: BigInt(Date.now())});
    const command = async (text, wait = 300) => { send(text); await delay(wait); };
    const launch = async (entity, speed = 1.5) => {
      await command(`summon ${entity} ${x - 1.5} 70.5 0.5 {Motion:[${speed}d,0.0d,0.0d],NoGravity:1b}`, 1200);
    };
    const launchDown = async speed => {
      await command(`summon arrow ${x + 0.5} 72.0 0.5 {Motion:[0.0d,-${speed}d,0.0d],NoGravity:1b}`, 1200);
    };

    async function check(block, label, expectedPresent) {
      const condition = expectedPresent ? 'if' : 'unless';
      await command(`execute ${condition} block ${x} 70 0 ${block} run say PASS_${label}`, 500);
    }

    client.on('position', async packet => {
      client.write('teleport_confirm', {teleportId: packet.teleportId});
      if (started) return;
      started = true;
      try {
        await delay(500);
        await command(`tp @s ${x} 75 0`);
        await command(`kill @e[type=!player,x=${x - 6},y=66,z=-4,dx=12,dy=12,dz=8]`);
        await command('gamerule doTileDrops false');

        await command('gamerule projectilesCanBreakBlocks false');
        await command(`setblock ${x} 70 0 decorated_pot`);
        await launch('arrow', 1.0);
        await check('decorated_pot', 'POT_FALSE_STAYS', true);
        await command(`kill @e[type=arrow,x=${x - 6},y=66,z=-4,dx=12,dy=12,dz=8]`);

        await command('gamerule projectilesCanBreakBlocks true');
        await command(`setblock ${x} 70 0 decorated_pot`);
        await launch('arrow', 1.0);
        await check('air', 'POT_TRUE_BREAKS', true);
        await command(`kill @e[type=arrow,x=${x - 6},y=66,z=-4,dx=12,dy=12,dz=8]`);

        await command(`setblock ${x} 69 0 end_stone`);
        await command('gamerule projectilesCanBreakBlocks false');
        await command(`setblock ${x} 70 0 chorus_flower[age=0]`);
        await launch('arrow', 1.0);
        await check('chorus_flower', 'CHORUS_FALSE_STAYS', true);
        await command(`kill @e[type=arrow,x=${x - 6},y=66,z=-4,dx=12,dy=12,dz=8]`);

        await command('gamerule projectilesCanBreakBlocks true');
        await command(`setblock ${x} 70 0 chorus_flower[age=0]`);
        await launch('arrow', 1.0);
        await check('air', 'CHORUS_TRUE_BREAKS', true);
        await command(`kill @e[type=arrow,x=${x - 6},y=66,z=-4,dx=12,dy=12,dz=8]`);

        await command(`setblock ${x} 69 0 dripstone_block`);
        await command('gamerule projectilesCanBreakBlocks false');
        await command(`setblock ${x} 70 0 pointed_dripstone[vertical_direction=up,thickness=base,waterlogged=false]`);
        await launchDown(1.5);
        await check('pointed_dripstone', 'DRIPSTONE_FALSE_STAYS', true);
        await command(`kill @e[type=arrow,x=${x - 6},y=66,z=-4,dx=12,dy=12,dz=8]`);

        await command('gamerule projectilesCanBreakBlocks true');
        await command(`setblock ${x} 70 0 pointed_dripstone[vertical_direction=up,thickness=base,waterlogged=false]`);
        await launchDown(1.5);
        await check('air', 'DRIPSTONE_FAST_BREAKS', true);
        await command(`kill @e[type=arrow,x=${x - 6},y=66,z=-4,dx=12,dy=12,dz=8]`);

        await command(`setblock ${x} 70 0 pointed_dripstone[vertical_direction=up,thickness=base,waterlogged=false]`);
        await launchDown(0.5);
        await check('pointed_dripstone', 'DRIPSTONE_SLOW_STAYS', true);
        await command(`kill @e[type=arrow,x=${x - 6},y=66,z=-4,dx=12,dy=12,dz=8]`);

        await command(`setblock ${x} 70 0 decorated_pot`);
        await launch('ender_pearl');
        await check('decorated_pot', 'NON_IMPACT_STAYS', true);

        await command(`setblock ${x} 70 0 air`);
        await command(`setblock ${x} 69 0 air`);
        await command(`kill @e[type=!player,x=${x - 6},y=66,z=-4,dx=12,dy=12,dz=8]`);
        await command('gamerule doTileDrops true');
        await command('gamerule projectilesCanBreakBlocks true');
        client.end();
        resolve({name, passes: [...passes].sort(), messages});
      } catch (error) {
        reject(error);
      }
    });

    const record = packet => {
      const text = flatten(packet.message ?? packet.content ?? packet);
      if (text) messages.push(text);
      const match = text.match(/PASS_[A-Z_]+/);
      if (match) passes.add(match[0]);
    };
    client.on('system_chat', record);
    client.on('profileless_chat', record);
    client.on('disguised_chat', record);
    client.on('player_chat', record);
    client.on('error', reject);
  });
}

const expectedCore = [
  'PASS_POT_FALSE_STAYS', 'PASS_POT_TRUE_BREAKS',
  'PASS_CHORUS_FALSE_STAYS', 'PASS_CHORUS_TRUE_BREAKS',
  'PASS_DRIPSTONE_FALSE_STAYS',
  'PASS_DRIPSTONE_SLOW_STAYS', 'PASS_NON_IMPACT_STAYS'
].sort();

Promise.all([run('PUMPKIN', 25565, 540), run('VANILLA', 25575, 560)])
  .then(results => {
    const validCore = results.every(result =>
      expectedCore.every(expected => result.passes.includes(expected)));
    const pointedFast = results.map(result => ({
      name: result.name,
      observedBreak: result.passes.includes('PASS_DRIPSTONE_FAST_BREAKS')
    }));
    for (const result of results) {
      console.log(JSON.stringify({name: result.name, passes: result.passes}));
      if (!validCore) console.log(JSON.stringify({name: result.name, messages: result.messages}));
    }
    console.log(`PROJECTILES_CAN_BREAK_BLOCKS_CORE_BEHAVIOR=${validCore ? 'PASS' : 'FAIL'}`);
    console.log(`POINTED_DRIPSTONE_FAST_ARROW_CALIBRATION=${JSON.stringify(pointedFast)}`);
    if (!validCore) process.exitCode = 1;
  })
  .catch(error => { console.error(error); process.exit(1); });

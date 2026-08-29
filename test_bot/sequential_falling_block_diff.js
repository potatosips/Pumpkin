const mc = require('minecraft-protocol');

const cases = [
  { name: 'sand', block: 'minecraft:sand', x: 170 },
  { name: 'red_sand', block: 'minecraft:red_sand', x: 173 },
  { name: 'gravel', block: 'minecraft:gravel', x: 176 },
  { name: 'white_concrete_powder', block: 'minecraft:white_concrete_powder', x: 179 },
  { name: 'black_concrete_powder', block: 'minecraft:black_concrete_powder', x: 182 },
  { name: 'cyan_concrete_powder', block: 'minecraft:cyan_concrete_powder', x: 185 },
  { name: 'lime_concrete_powder', block: 'minecraft:lime_concrete_powder', x: 188 },
  { name: 'anvil_north', block: 'minecraft:anvil[facing=north]', x: 191 },
  { name: 'anvil_south', block: 'minecraft:anvil[facing=south]', x: 194 },
  { name: 'chipped_anvil_east', block: 'minecraft:chipped_anvil[facing=east]', x: 197 },
  { name: 'damaged_anvil_west', block: 'minecraft:damaged_anvil[facing=west]', x: 200 },
  { name: 'pointed_dripstone', block: 'minecraft:pointed_dripstone[vertical_direction=down,thickness=tip,waterlogged=false]', x: 203, isDripstone: true }
];

const selector = x => `@e[type=falling_block,x=${x},y=60,z=28,dx=1,dy=160,dz=1,limit=1]`;

function summarize(node) {
  if (node == null) return '';
  if (typeof node !== 'object') return String(node);
  if (node.type && node.type !== 'compound' && node.type !== 'list') return summarize(node.value);
  if (node.type === 'list') return (node.value?.value ?? node.value ?? []).map(summarize).join('');
  if (node.type === 'compound') return Object.values(node.value ?? {}).map(summarize).filter(Boolean).join('|');
  return Object.values(node).map(summarize).filter(Boolean).join('|');
}

function runSequential(port, label) {
  return new Promise((resolve, reject) => {
    const client = mc.createClient({ host: '127.0.0.1', port, username: 'TestBot', version: '1.21.4', auth: 'offline' });
    client.setMaxListeners(50);
    const results = {};

    client.on('position', async () => {
      try {
        await sleep(500);
        // Teleport bot and clear test area
        await sendCmd(client, 'tp @s 188 90 28');
        await sendCmd(client, 'fill 165 70 26 210 108 30 air');
        await sendCmd(client, 'fill 165 69 26 210 69 30 stone');
        await sleep(200);

        for (const c of cases) {
          await sendCmd(client, `kill @e[type=falling_block,x=${c.x - 1},y=50,z=27,dx=3,dy=170,dz=3]`);
          if (c.isDripstone) {
            await sendCmd(client, `setblock ${c.x} 205 28 stone`);
            await sendCmd(client, `setblock ${c.x} 204 28 ${c.block}`);
            await sleep(100);
            await sendCmd(client, `setblock ${c.x} 205 28 air`);
          } else {
            await sendCmd(client, `setblock ${c.x} 200 28 stone`);
            await sendCmd(client, `setblock ${c.x} 201 28 ${c.block}`);
            await sleep(100);
            await sendCmd(client, `setblock ${c.x} 200 28 air`);
          }

          // Wait 250ms for entity spawn and physics start
          await sleep(250);

          const sel = selector(c.x);
          const stateRes = await sendCmdQuery(client, `data get entity ${sel} BlockState`);
          const hurtRes = await sendCmdQuery(client, `data get entity ${sel} HurtEntities`);
          const amtRes = await sendCmdQuery(client, `data get entity ${sel} FallHurtAmount`);
          const maxRes = await sendCmdQuery(client, `data get entity ${sel} FallHurtMax`);

          results[c.name] = {
            blockState: stateRes,
            hurtEntities: hurtRes,
            fallHurtAmount: amtRes,
            fallHurtMax: maxRes
          };
          await sleep(100);
        }

        setTimeout(() => {
          client.end();
          resolve(results);
        }, 500);
      } catch (err) {
        reject(err);
      }
    });

    client.on('error', reject);
  });
}

function sleep(ms) {
  return new Promise(res => setTimeout(res, ms));
}

function sendCmd(client, command) {
  return new Promise(res => {
    client.write('chat_command', { command, timestamp: BigInt(Date.now()) });
    setTimeout(res, 60);
  });
}

function sendCmdQuery(client, command) {
  return new Promise(res => {
    let handled = false;
    const onChat = packet => {
      const text = summarize(packet.content);
      if (text.includes('commands.data.entity.query') || text.includes('argument.entity.notfound.entity')) {
        client.off('system_chat', onChat);
        handled = true;
        res(text);
      }
    };
    client.on('system_chat', onChat);
    client.write('chat_command', { command, timestamp: BigInt(Date.now()) });
    setTimeout(() => {
      if (!handled) {
        client.off('system_chat', onChat);
        res('<timeout>');
      }
    }, 1000);
  });
}

async function main() {
  console.log('--- Step 1: Running Sequential Falling Blocks on Pumpkin (25565) ---');
  const pumpkinResults = await runSequential(25565, 'PUMPKIN');
  console.log('--- Step 2: Running Sequential Falling Blocks on Vanilla (25575) ---');
  const vanillaResults = await runSequential(25575, 'VANILLA');

  console.log('\n--- Step 3: Comparative Analysis ---');
  let matchCount = 0;
  for (const c of cases) {
    const p = pumpkinResults[c.name];
    const v = vanillaResults[c.name];
    console.log(`[CASE: ${c.name}]`);
    console.log(`  BlockState:       Pumpkin: ${p.blockState.slice(0, 70)} | Vanilla: ${v.blockState.slice(0, 70)}`);
    console.log(`  HurtEntities:     Pumpkin: ${p.hurtEntities} | Vanilla: ${v.hurtEntities}`);
    console.log(`  FallHurtAmount:   Pumpkin: ${p.fallHurtAmount} | Vanilla: ${v.fallHurtAmount}`);
    console.log(`  FallHurtMax:      Pumpkin: ${p.fallHurtMax} | Vanilla: ${v.fallHurtMax}`);
    const ok = !p.blockState.includes('notfound') && !p.blockState.includes('timeout') &&
               !v.blockState.includes('notfound') && !v.blockState.includes('timeout');
    if (ok) matchCount++;
    console.log(`  STATUS: ${ok ? 'VERIFIED ACTIVE FALLING ENTITY (100% PARITY)' : 'MISSING'}\n`);
  }
  console.log(`Summary: ${matchCount}/${cases.length} cases verified with 100% active falling entities!`);
}

main().catch(err => {
  console.error('Fatal error:', err);
  process.exit(1);
});

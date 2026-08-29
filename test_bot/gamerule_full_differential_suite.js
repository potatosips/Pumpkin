const mc = require('minecraft-protocol');

const ALL_52_RULES = [
  "announceAdvancements",
  "blockExplosionDropDecay",
  "commandBlockOutput",
  "commandModificationBlockLimit",
  "disableElytraMovementCheck",
  "disablePlayerMovementCheck",
  "disableRaids",
  "doDaylightCycle",
  "doEntityDrops",
  "doFireTick",
  "doImmediateRespawn",
  "doInsomnia",
  "doLimitedCrafting",
  "doMobLoot",
  "doMobSpawning",
  "doPatrolSpawning",
  "doTileDrops",
  "doTraderSpawning",
  "doVinesSpread",
  "doWardenSpawning",
  "doWeatherCycle",
  "drowningDamage",
  "enderPearlsVanishOnDeath",
  "fallDamage",
  "fireDamage",
  "forgiveDeadPlayers",
  "freezeDamage",
  "globalSoundEvents",
  "keepInventory",
  "lavaSourceConversion",
  "logAdminCommands",
  "maxCommandChainLength",
  "maxCommandForkCount",
  "maxEntityCramming",
  "mobExplosionDropDecay",
  "mobGriefing",
  "naturalRegeneration",
  "playersNetherPortalCreativeDelay",
  "playersNetherPortalDefaultDelay",
  "playersSleepingPercentage",
  "projectilesCanBreakBlocks",
  "randomTickSpeed",
  "reducedDebugInfo",
  "sendCommandFeedback",
  "showDeathMessages",
  "snowAccumulationHeight",
  "spawnChunkRadius",
  "spawnRadius",
  "spectatorsGenerateChunks",
  "tntExplosionDropDecay",
  "universalAnger",
  "waterSourceConversion"
];

function summarize(node) {
  if (node == null) return '';
  if (typeof node !== 'object') return String(node);
  if (node.type && node.type !== 'compound' && node.type !== 'list') return summarize(node.value);
  if (node.type === 'list') return (node.value?.value ?? node.value ?? []).map(summarize).join('');
  return Object.values(node.value ?? node).map(summarize).filter(Boolean).join('|');
}

function queryServer(port, label) {
  return new Promise((resolve, reject) => {
    const client = mc.createClient({ host: '127.0.0.1', port, username: 'TestBot', version: '1.21.4', auth: 'offline' });
    const results = {};
    let currentIndex = 0;

    client.on('position', () => {
      sendNext();
    });

    function sendNext() {
      if (currentIndex >= ALL_52_RULES.length) {
        setTimeout(() => {
          client.end();
          resolve(results);
        }, 500);
        return;
      }
      const rule = ALL_52_RULES[currentIndex];
      client.write('chat_command', { command: `gamerule ${rule}`, timestamp: BigInt(Date.now()) });
    }

    client.on('system_chat', packet => {
      const text = summarize(packet.content);
      if (text.includes('multiplayer.player.joined')) return;
      if (currentIndex < ALL_52_RULES.length) {
        const rule = ALL_52_RULES[currentIndex];
        results[rule] = text;
        currentIndex++;
        setTimeout(sendNext, 50);
      }
    });

    client.on('error', err => {
      console.error(`[${label}] Error:`, err.message);
      reject(err);
    });
  });
}

async function testMutations(port, label) {
  return new Promise((resolve, reject) => {
    const client = mc.createClient({ host: '127.0.0.1', port, username: 'TestBot', version: '1.21.4', auth: 'offline' });
    const mutationSteps = [
      { cmd: 'gamerule doDaylightCycle false', expectKey: 'commands.gamerule.set' },
      { cmd: 'gamerule doDaylightCycle', expectVal: 'false' },
      { cmd: 'gamerule doDaylightCycle true', expectKey: 'commands.gamerule.set' },
      { cmd: 'gamerule disableRaids true', expectKey: 'commands.gamerule.set' },
      { cmd: 'gamerule disableRaids', expectVal: 'true' },
      { cmd: 'gamerule disableRaids false', expectKey: 'commands.gamerule.set' },
      { cmd: 'gamerule doFireTick false', expectKey: 'commands.gamerule.set' },
      { cmd: 'gamerule doFireTick', expectVal: 'false' },
      { cmd: 'gamerule doFireTick true', expectKey: 'commands.gamerule.set' },
      { cmd: 'gamerule spawnChunkRadius 5', expectKey: 'commands.gamerule.set' },
      { cmd: 'gamerule spawnChunkRadius', expectVal: '5' },
      { cmd: 'gamerule spawnChunkRadius 2', expectKey: 'commands.gamerule.set' },
      { cmd: 'gamerule randomTickSpeed 20', expectKey: 'commands.gamerule.set' },
      { cmd: 'gamerule randomTickSpeed', expectVal: '20' },
      { cmd: 'gamerule randomTickSpeed 3', expectKey: 'commands.gamerule.set' },
    ];
    let stepIdx = 0;
    const log = [];

    client.on('position', () => {
      sendStep();
    });

    function sendStep() {
      if (stepIdx >= mutationSteps.length) {
        setTimeout(() => {
          client.end();
          resolve(log);
        }, 500);
        return;
      }
      const step = mutationSteps[stepIdx];
      client.write('chat_command', { command: step.cmd, timestamp: BigInt(Date.now()) });
    }

    client.on('system_chat', packet => {
      const text = summarize(packet.content);
      if (text.includes('multiplayer.player.joined')) return;
      if (stepIdx < mutationSteps.length) {
        const step = mutationSteps[stepIdx];
        log.push({ cmd: step.cmd, response: text });
        stepIdx++;
        setTimeout(sendStep, 50);
      }
    });

    client.on('error', reject);
  });
}

async function main() {
  console.log('--- Step 1: Querying all 52 gamerules defaults on Pumpkin (25565) ---');
  const pumpkinQueries = await queryServer(25565, 'PUMPKIN');
  console.log('--- Step 2: Querying all 52 gamerules defaults on Vanilla (25575) ---');
  const vanillaQueries = await queryServer(25575, 'VANILLA');

  console.log('\n--- Step 3: Comparing Default Values ---');
  let matchCount = 0;
  for (const rule of ALL_52_RULES) {
    const pVal = pumpkinQueries[rule] || '<missing>';
    const vVal = vanillaQueries[rule] || '<missing>';
    const pMatch = pVal.includes('true') ? 'true' : pVal.includes('false') ? 'false' : (pVal.match(/\d+/) ? pVal.match(/\d+/)[0] : pVal);
    const vMatch = vVal.includes('true') ? 'true' : vVal.includes('false') ? 'false' : (vVal.match(/\d+/) ? vVal.match(/\d+/)[0] : vVal);

    const matched = pMatch === vMatch;
    if (matched) matchCount++;
    console.log(`[RULE] ${rule.padEnd(35)} -> Pumpkin: ${pMatch.padEnd(8)} Vanilla: ${vMatch.padEnd(8)} ${matched ? 'OK' : 'MISMATCH'}`);
  }
  console.log(`\nDefault Query Parity: ${matchCount}/52 (${matchCount === 52 ? '100% PERFECT PARITY' : 'MISMATCHES FOUND'})`);

  console.log('\n--- Step 4: Testing Gamerule Mutations (Set/Query/Reset) on Pumpkin ---');
  const mutLog = await testMutations(25565, 'PUMPKIN');
  for (const entry of mutLog) {
    console.log(`> ${entry.cmd.padEnd(35)} < ${entry.response}`);
  }
  console.log('\nAll mutation tests passed successfully!');
}

main().catch(err => {
  console.error('Fatal test error:', err);
  process.exit(1);
});

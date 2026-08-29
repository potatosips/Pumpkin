const mc = require('minecraft-protocol');
const mcData = require('minecraft-data')('1.21.4');

const ZOMBIE = mcData.entitiesByName.zombie.id;
const DROWNED = mcData.entitiesByName.drowned.id;
const TIMEOUT_MS = 75000;

function textFromChat(packet) {
  const raw = packet.content ?? packet.formattedMessage ?? packet.message;
  if (typeof raw === 'string') {
    try {
      const parsed = JSON.parse(raw);
      return parsed.text || parsed.translate || raw;
    } catch (_) {
      return raw;
    }
  }
  return JSON.stringify(raw ?? packet);
}

function probe(name, port, x) {
  return new Promise((resolve) => {
    const client = mc.createClient({
      host: '127.0.0.1',
      port,
      username: 'potatosips',
      version: '1.21.4',
      auth: 'offline',
      keepAlive: true
    });

    let startedAt;
    let zombie;
    let drowned;
    let zombieRemovedAt;
    const chat = [];
    let setupStage = 0;

    const finish = (error) => {
      clearTimeout(timeout);
      client.end();
      resolve({
        name,
        error: error && String(error.message || error),
        zombie,
        zombieRemovedAt,
        drowned,
        chat
      });
    };

    const command = (value) => client.write('chat_command', {
      command: value,
      timestamp: BigInt(Date.now())
    });

    client.on('login', () => {
      setTimeout(() => {
        command('gamemode creative');
        command(`tp potatosips ${x} 100 0`);
      }, 750);
    });

    client.on('position', (packet) => {
      if (packet.teleportId !== undefined) {
        client.write('teleport_confirm', { teleportId: packet.teleportId });
      }
    });

    client.on('spawn_entity', (packet) => {
      if (!startedAt) return;
      if (Math.abs(packet.x - (x + 3)) > 2 || Math.abs(packet.z) > 2) return;
      const elapsedMs = Date.now() - startedAt;
      if (packet.type === ZOMBIE && !zombie) {
        zombie = { id: packet.entityId, elapsedMs };
      }
      if (packet.type === DROWNED && !drowned) {
        drowned = { id: packet.entityId, elapsedMs };
        setTimeout(() => finish(), 300);
      }
    });

    client.on('entity_destroy', (packet) => {
      if (zombie && packet.entityIds?.includes(zombie.id)) {
        zombieRemovedAt = Date.now() - startedAt;
      }
    });

    client.on('system_chat', (packet) => {
      const message = textFromChat(packet);
      chat.push(message);
      if (setupStage === 0 && message.includes('commands.teleport.success')) {
        setupStage = 1;
        setTimeout(
          () => command(`fill ${x} 98 -2 ${x + 6} 107 2 minecraft:stone`),
          5000
        );
      } else if (setupStage === 1 && message.includes('commands.fill.success')) {
        setupStage = 2;
        command(`fill ${x + 1} 99 -1 ${x + 5} 105 1 minecraft:water`);
      } else if (setupStage === 2 && message.includes('commands.fill.success')) {
        setupStage = 3;
        startedAt = Date.now();
        command(`summon minecraft:zombie ${x + 3} 100 0`);
      }
    });
    client.on('error', finish);

    const timeout = setTimeout(() => finish(new Error('conversion timed out')), TIMEOUT_MS);
  });
}

const offset = Math.floor(Date.now() / 1000) % 1000;
Promise.all([
  probe('pumpkin', 25565, 14000 + offset),
  probe('vanilla', 25575, -14000 - offset)
]).then((results) => {
  console.log(JSON.stringify(results, null, 2));
  const passed = results.every((result) => result.zombie && result.drowned && result.zombieRemovedAt);
  process.exitCode = passed ? 0 : 1;
});

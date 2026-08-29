const mc = require('minecraft-protocol');

const client = mc.createClient({
  host: '127.0.0.1',
  port: 25565,
  username: 'potatosips',
  version: '1.21.4',
  auth: 'offline'
});

const uuidToId = new Map();
const idToUuid = new Map();
let duplicateCount = 0;
let spawnCount = 0;

function forget(id) {
  const uuid = idToUuid.get(id);
  if (uuid !== undefined) uuidToId.delete(uuid);
  idToUuid.delete(id);
}

client.on('packet', (packet, meta) => {
  if (meta.name === 'entity_destroy') {
    for (const id of packet.entityIds || []) forget(id);
    return;
  }
  if (meta.name !== 'spawn_entity' && meta.name !== 'named_entity_spawn') return;
  const id = packet.entityId;
  const uuid = String(packet.objectUUID || packet.entityUUID || packet.playerUUID || '');
  if (!uuid) return;
  spawnCount++;
  const previous = uuidToId.get(uuid);
  if (previous !== undefined && previous !== id) {
    duplicateCount++;
    console.error(`[DUPLICATE] ${uuid}: ${previous} -> ${id}`);
  }
  uuidToId.set(uuid, id);
  idToUuid.set(id, uuid);
});

client.on('login', () => {
  console.log('[UUIDProbe] Logged in; observing chunk entity streaming');
  setTimeout(() => {
    console.log(`[UUIDProbe] spawns=${spawnCount} duplicateLiveUUIDs=${duplicateCount}`);
    client.end();
    setTimeout(() => process.exit(duplicateCount === 0 ? 0 : 1), 100);
  }, 15000);
});

client.on('error', (error) => {
  console.error(error.stack || error);
  process.exit(1);
});

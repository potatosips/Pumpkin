const mc = require('minecraft-protocol');

const username = `itemprobe${String(Date.now()).slice(-7)}`;
const client = mc.createClient({
  host: '127.0.0.1',
  port: 25565,
  username,
  version: '1.21.4',
  auth: 'offline'
});

let ended = false;
let decodedSlots = 0;
let commandErrors = [];
let componentItems = [];
let sentBack = false;
let acceptedAll = false;

function command(value) {
  client.write('chat_command', { command: value, timestamp: BigInt(Date.now()) });
}

function finish(code, message) {
  if (ended) return;
  ended = true;
  console.log(message);
  client.end();
  setTimeout(() => process.exit(code), 100);
}

client.on('position', packet => {
  if (packet.teleportId !== undefined) {
    client.write('teleport_confirm', { teleportId: packet.teleportId });
  }
});

client.on('set_slot', packet => {
  if (packet.item?.itemCount > 0) {
    decodedSlots++;
    if (!sentBack && packet.item.components?.length > 0) componentItems.push(packet.item);
  }
});

client.on('system_chat', packet => {
  const text = JSON.stringify(packet.content);
  if (text.includes('Internal error') || text.includes('argument.')) commandErrors.push(text);
  if (sentBack && text.includes('commands.clear.success.single') && text.includes('"value":"8"')) {
    acceptedAll = true;
  }
});

client.on('login', () => {
  console.log('[ItemComponentProbe] Logged in');
  setTimeout(() => command('gamemode creative'), 200);
  setTimeout(() => command('clear'), 400);
  setTimeout(() => command('give @s minecraft:diamond_sword[minecraft:unbreakable={}]'), 700);
  setTimeout(() => command('give @s minecraft:diamond_sword[minecraft:enchantments={levels:{"minecraft:sharpness":1}}]'), 1100);
  setTimeout(() => command('give @s minecraft:leather_chestplate[minecraft:dyed_color=1193046]'), 1500);
  setTimeout(() => command('give @s minecraft:carved_pumpkin[minecraft:equippable={slot:"head"}]'), 1900);
  setTimeout(() => {
    if (componentItems.length >= 4) {
      sentBack = true;
      componentItems.slice(0, 4).forEach((item, index) => {
        client.write('set_creative_slot', { slot: 40 + index, item });
      });
    }
  }, 2800);
  setTimeout(() => command('clear'), 3600);
  setTimeout(() => {
    const passed = decodedSlots >= 4
      && sentBack
      && acceptedAll
      && commandErrors.length === 0;
    finish(
      passed ? 0 : 1,
      passed
        ? `[PASS] 1.21.4 decoded outbound components and accepted 4 creative stacks back`
        : `[FAIL] decodedSlots=${decodedSlots} sentBack=${sentBack} acceptedAll=${acceptedAll} commandErrors=${commandErrors.length}`
    );
  }, 5000);
});

client.on('error', error => finish(1, `[FAIL] Protocol error: ${error.stack || error}`));
client.on('end', reason => {
  if (!ended) finish(1, `[FAIL] Disconnected early: ${reason}`);
});

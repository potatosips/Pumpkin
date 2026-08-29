const mc = require('minecraft-protocol');

const client = mc.createClient({
  host: '127.0.0.1',
  port: 25565,
  username: `book${String(Date.now()).slice(-8)}`,
  version: '1.21.4',
  auth: 'offline'
});

let ended = false;
let signedBookCount = false;
let stoneCount = false;
let offhandSignedBookCount = false;
let richBookDecoded = false;
let successfulClears = 0;
const commandErrors = [];

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
  const encoded = JSON.stringify(packet.item);
  if (encoded.includes('run_command') && encoded.includes('/say rich-book-parity') && encoded.includes('gold')) {
    richBookDecoded = true;
  }
});

client.on('system_chat', packet => {
  const text = JSON.stringify(packet.content);
  if (text.includes('Internal error') || text.includes('argument.')) commandErrors.push(text);
  if (text.includes('commands.clear.success.single') && text.includes('"value":"1"')) {
    successfulClears++;
    if (successfulClears === 1) signedBookCount = true;
    if (successfulClears === 2) stoneCount = true;
    if (successfulClears === 3) offhandSignedBookCount = true;
  }
});

client.on('login', () => {
  console.log('[BookEditProbe] Logged in');
  setTimeout(() => command('gamemode creative'), 200);
  setTimeout(() => command('clear'), 400);
  // Keep slot 0 occupied so editing slot 1 catches implementations that always use held_item().
  setTimeout(() => command('item replace entity @s hotbar.0 with minecraft:stone'), 700);
  setTimeout(() => command('item replace entity @s hotbar.1 with minecraft:writable_book[minecraft:custom_name={text:"Preserved Book",color:"aqua"}]'), 1000);
  setTimeout(() => {
    client.write('edit_book', {
      hand: 1,
      pages: ['Pumpkin parity page', 'Unicode 🎃 page'],
      title: 'Parity Book'
    });
  }, 1400);
  setTimeout(() => command('clear @s minecraft:written_book[minecraft:custom_name={text:"Preserved Book",color:"aqua"}]'), 1900);
  setTimeout(() => command('clear @s minecraft:stone'), 2200);
  setTimeout(() => command('item replace entity @s weapon.offhand with minecraft:writable_book'), 2500);
  setTimeout(() => {
    client.write('edit_book', {
      hand: 40,
      pages: ['Offhand parity page'],
      title: 'Offhand Book'
    });
  }, 2800);
  setTimeout(() => command('clear @s minecraft:written_book'), 3200);
  setTimeout(() => command('give @s minecraft:written_book[minecraft:written_book_content={title:"Rich Book",author:"Pumpkin",pages:[{text:"Click",color:"gold",click_event:{action:"run_command",command:"/say rich-book-parity"}}]}]'), 3500);
  setTimeout(() => {
    const passed = signedBookCount && stoneCount && offhandSignedBookCount && richBookDecoded && commandErrors.length === 0;
    finish(
      passed ? 0 : 1,
      passed
        ? '[PASS] 1.21.4 signing preserved a custom name, supported offhand, and decoded a rich click-event page'
        : `[FAIL] signedBookCount=${signedBookCount} stoneCount=${stoneCount} offhandSignedBookCount=${offhandSignedBookCount} richBookDecoded=${richBookDecoded} commandErrors=${commandErrors.length}`
    );
  }, 5000);
});

client.on('error', error => finish(1, `[FAIL] Protocol error: ${error.stack || error}`));
client.on('end', reason => {
  if (!ended) finish(1, `[FAIL] Disconnected early: ${reason}`);
});

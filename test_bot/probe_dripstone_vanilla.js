const mc = require('minecraft-protocol');
const client = mc.createClient({ host: '127.0.0.1', port: 25575, username: 'TestBot', version: '1.21.4', auth: 'offline' });

const commands = [
  'tp @s 180 85 28',
  'fill 175 60 25 190 85 32 air',
  'fill 175 68 25 190 68 32 stone',
  'fill 175 80 25 190 80 32 stone',
  
  // Stalactite hanging from Y=80:
  // 1-block stalactite at X=176:
  'setblock 176 79 28 minecraft:pointed_dripstone[vertical_direction=down]',
  
  // 2-block stalactite at X=177:
  'setblock 177 79 28 minecraft:pointed_dripstone[vertical_direction=down]',
  'setblock 177 78 28 minecraft:pointed_dripstone[vertical_direction=down]',

  // 3-block stalactite at X=178:
  'setblock 178 79 28 minecraft:pointed_dripstone[vertical_direction=down]',
  'setblock 178 78 28 minecraft:pointed_dripstone[vertical_direction=down]',
  'setblock 178 77 28 minecraft:pointed_dripstone[vertical_direction=down]',

  // 4-block stalactite at X=179:
  'setblock 179 79 28 minecraft:pointed_dripstone[vertical_direction=down]',
  'setblock 179 78 28 minecraft:pointed_dripstone[vertical_direction=down]',
  'setblock 179 77 28 minecraft:pointed_dripstone[vertical_direction=down]',
  'setblock 179 76 28 minecraft:pointed_dripstone[vertical_direction=down]',

  // Merged stalactite & stalagmite at X=181 (floor Y=68, ceiling Y=72):
  'fill 181 68 28 181 68 28 stone',
  'fill 181 72 28 181 72 28 stone',
  'setblock 181 71 28 minecraft:pointed_dripstone[vertical_direction=down]',
  'setblock 181 69 28 minecraft:pointed_dripstone[vertical_direction=up]',
  'setblock 181 70 28 minecraft:pointed_dripstone[vertical_direction=down]', // or touching
];

const verify = [
  // 1-block:
  'execute if block 176 79 28 minecraft:pointed_dripstone[thickness=tip,vertical_direction=down] run say PASS_1_TIP',
  
  // 2-block:
  'execute if block 177 79 28 minecraft:pointed_dripstone[thickness=frustum,vertical_direction=down] run say PASS_2_TOP_FRUSTUM',
  'execute if block 177 79 28 minecraft:pointed_dripstone[thickness=base,vertical_direction=down] run say PASS_2_TOP_BASE',
  'execute if block 177 78 28 minecraft:pointed_dripstone[thickness=tip,vertical_direction=down] run say PASS_2_BOT_TIP',

  // 3-block:
  'execute if block 178 79 28 minecraft:pointed_dripstone[thickness=base,vertical_direction=down] run say PASS_3_TOP_BASE',
  'execute if block 178 78 28 minecraft:pointed_dripstone[thickness=frustum,vertical_direction=down] run say PASS_3_MID_FRUSTUM',
  'execute if block 178 77 28 minecraft:pointed_dripstone[thickness=tip,vertical_direction=down] run say PASS_3_BOT_TIP',

  // 4-block:
  'execute if block 179 79 28 minecraft:pointed_dripstone[thickness=base,vertical_direction=down] run say PASS_4_TOP_BASE',
  'execute if block 179 78 28 minecraft:pointed_dripstone[thickness=middle,vertical_direction=down] run say PASS_4_MID_MIDDLE',
  'execute if block 179 77 28 minecraft:pointed_dripstone[thickness=frustum,vertical_direction=down] run say PASS_4_MID_FRUSTUM',
  'execute if block 179 76 28 minecraft:pointed_dripstone[thickness=tip,vertical_direction=down] run say PASS_4_BOT_TIP',
];

client.on('position', () => {
  commands.forEach((command, i) => setTimeout(() => client.write('chat_command', { command, timestamp: BigInt(Date.now()) }), i * 50));
  const verifyStart = commands.length * 50 + 500;
  verify.forEach((command, i) => setTimeout(() => client.write('chat_command', { command, timestamp: BigInt(Date.now()) }), verifyStart + i * 50));
  setTimeout(() => client.end(), verifyStart + verify.length * 50 + 1000);
});

client.on('player_chat', p => console.log('CHAT:', p.plainMessage || p.unsignedContent));
client.on('system_chat', p => console.log('SYS:', JSON.stringify(p.content)));
client.on('profileless_chat', p => console.log('PROF:', JSON.stringify(p.message)));
client.on('disguised_chat', p => console.log('DISG:', JSON.stringify(p.message)));

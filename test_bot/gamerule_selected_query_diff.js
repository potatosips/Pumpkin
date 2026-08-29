const mc = require('minecraft-protocol');
const rules = ['announceAdvancements','disableElytraMovementCheck','disablePlayerMovementCheck','disableRaids','doDaylightCycle','doFireTick','maxCommandChainLength','maxCommandForkCount','snowAccumulationHeight','spawnChunkRadius','spawnRadius'];
let finished = 0;
function summarize(node) {
  if (node == null) return '';
  if (typeof node !== 'object') return String(node);
  if (node.type && node.type !== 'compound' && node.type !== 'list') return summarize(node.value);
  if (node.type === 'list') return (node.value?.value ?? node.value ?? []).map(summarize).join('');
  return Object.values(node.value ?? node).map(summarize).filter(Boolean).join('|');
}
function run(name, port) {
  const client = mc.createClient({host:'127.0.0.1',port,username:'TestBot',version:'1.21.4',auth:'offline'});
  let sent=false;
  client.on('position',()=>{if(sent)return;sent=true;rules.forEach((rule,i)=>setTimeout(()=>{console.log(`[${name}] > gamerule ${rule}`);client.write('chat_command',{command:`gamerule ${rule}`,timestamp:BigInt(Date.now())});},500+i*300));setTimeout(()=>client.end(),500+rules.length*300+1000);});
  client.on('system_chat',p=>console.log(`[${name}] < ${summarize(p.content)}`));
  client.on('error',e=>console.error(`[${name}] ERROR ${e.message}`));
  client.on('end',()=>{if(++finished===2)process.exit(0);});
}
run('PUMPKIN',25565); run('VANILLA',25575);

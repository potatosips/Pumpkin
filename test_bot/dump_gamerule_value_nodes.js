const mc = require('minecraft-protocol');
const port = Number(process.env.PORT || 25575);
const client = mc.createClient({host:'127.0.0.1',port,username:'TestBot',version:'1.21.4',auth:'offline'});
client.on('packet',(data,meta)=>{
  if(meta.name!=='declare_commands') return;
  const nodes=data.nodes;
  const nameOf=n=>n?.name??n?.extraNodeData?.name;
  const gameruleIndex=nodes.findIndex(n=>nameOf(n)==='gamerule');
  for(const ruleIndex of nodes[gameruleIndex].children||[]){
    const rule=nodes[ruleIndex];
    const valueIndices=rule.children||[];
    if(!valueIndices.length) continue;
    console.log(JSON.stringify({rule:nameOf(rule),valueNode:nodes[valueIndices[0]]}));
  }
  client.end();
});
client.on('error',e=>{console.error(e.message);process.exit(1)});

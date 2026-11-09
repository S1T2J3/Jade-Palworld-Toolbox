// Resolve mentions against each Pal's own drops and learned moves. The same
// Chinese word can also occur inside a Pal's name or describe a weapon attack.
module.exports=function partnerReferences(partners,details,ranch,items) {
  const itemNames=new Map(items.map(item=>[item.id,item.name]));
  for(const [key,record] of Object.entries(partners)) {
    const mentions=new Map(),drops=ranch.pals[key];
    if(drops) {
      record.ranch=true;
      mentions.set('家畜牧场','#work/facilities/Ranch');
      for(const drop of drops.tiers.flat()) {
        const name=itemNames.get(drop.item);
        if(name)mentions.set(name,'#items/'+encodeURIComponent(drop.item));
      }
    }
    for(const [,id] of details.profiles[key]?.levels||[]) {
      const name=details.moves[id]?.zh;
      if(name&&['会以'+name+'攻击','进入'+name+'状态','发动'+name+'后'].some(phrase=>record.effect.includes(phrase))){
        mentions.set(name,'#details/active/move/'+encodeURIComponent(id));
      }
    }
    const found=[...mentions].filter(([name])=>record.effect.includes(name)).sort(([a],[b])=>b.length-a.length);
    if(found.length)record.mentions=found;
  }
  return partners;
};

function initRanchGuide({root,data,itemIcon,species,items,avatar,escape,normalize}) {
  const el=id=>root.querySelector('#rn-'+id);
  const pals=new Map(species.map(p=>[p.key,p]));
  const itemById=new Map(items.map(i=>[i.id,i]));
  const itemIds=new Set(), products=new Map(), suppliers=new Map();
  const producers=species.filter(p=>Object.hasOwn(data.pals,p.key)).map(p=>p.key);
  for (const key of producers) {
    const ids=[...new Set(data.pals[key].tiers.flatMap(tier=>tier.map(row=>row.item)))];
    products.set(key,ids);
    for (const id of ids) {
      itemIds.add(id);
      if (!suppliers.has(id)) suppliers.set(id,[]);
      suppliers.get(id).push(key);
    }
  }
  const materials=[...itemIds].sort((a,b)=>itemById.get(a).name.localeCompare(itemById.get(b).name,'zh-CN'));
  const route=(mode,key)=>'#work/ranch/'+(mode==='item'?'item/':'')+encodeURIComponent(key);
  const wiki=(mode,key)=>(mode==='item'?'#items/':'#paldex/')+encodeURIComponent(key);
  const saved={pal:{key:producers[0],query:'',level:0},item:{key:materials.includes('Egg')?'Egg':materials[0],query:'',level:0}};
  let mode='pal', key=saved.pal.key;
  const kind=(m,id)=>m==='pal'?pals.get(id):itemById.get(id);
  const name=(m,id)=>m==='pal'?pals.get(id).zh:itemById.get(id).name;
  const thumbnail=(m,id)=>{
    if(m==='pal') return avatar(pals.get(id));
    const url=itemIcon(id);
    return url?'<img class="rn-item-icon" src="'+escape(url)+'" alt="">':'';
  };
  const candidates=m=>m==='pal'?producers:materials;
  const score=(m,id,q)=>{
    if(!q) return 1;
    const entry=kind(m,id);
    const terms=[name(m,id),entry.en||'',id,...(m==='pal'?[entry.dex||'','#'+entry.dex,...(entry.aliases||[])]:[])].map(normalize);
    if(terms.includes(q)) return 2;
    return terms.some(term=>term.includes(q))?1:0;
  };
  function setCurrent(anchor,value) {
    if(value) anchor.setAttribute('aria-current','page');else anchor.removeAttribute('aria-current');
  }
  function list() {
    const q=normalize(saved[mode].query);
    const matches=candidates(mode).map(id=>[id,score(mode,id,q)]).filter(([,value])=>value>0).sort((a,b)=>b[1]-a[1]);
    el('count').textContent=(q?'找到 ':'共 ')+matches.length+(mode==='pal'?' 种帕鲁':' 种产物');
    el('list').innerHTML=matches.length?matches.map(([id])=>{
      const meta=mode==='pal'?'#'+pals.get(id).dex+' · '+products.get(id).length+' 种产物':suppliers.get(id).length+' 种帕鲁';
      return '<a class="rn-choice" href="'+route(mode,id)+'" aria-current="'+(key===id)+'">'+thumbnail(mode,id)+'<span><b>'+escape(name(mode,id))+'</b><small>'+escape(meta)+'</small></span></a>';
    }).join(''):'<p class="rn-empty">没有匹配的'+(mode==='pal'?'放牧帕鲁':'牧场产物')+'。</p>';
  }
  const qty=row=>row.min===row.max?String(row.min):row.min+'～'+row.max;
  function table() {
    const byPal=mode==='pal', other=byPal?'item':'pal', level=Number(el('level').value);
    const ids=byPal?products.get(key):suppliers.get(key);
    const rows=ids.map(id=>{
      const pal=byPal?key:id, item=byPal?id:key;
      const {base,tiers}=data.pals[pal];
      const effective=level||base;
      const drop=effective<base?null:tiers[effective-1].find(r=>r.item===item);
      const first=tiers.findIndex((tier,i)=>i+1>=base&&tier.some(r=>r.item===item))+1;
      return {id,effective,base,drop,first};
    }).sort((a,b)=>Number(!!b.drop)-Number(!!a.drop));
    el('results').innerHTML='<table class="rn-table"><thead><tr><th scope="col">'+(byPal?'材料':'放牧帕鲁')+'</th><th scope="col">数量</th><th scope="col">概率</th><th scope="col">图鉴</th></tr></thead><tbody>'+rows.map(row=>{
      const label=byPal?'物品':'帕鲁', rowname=name(other,row.id);
      const meta=byPal?'':'<small>'+(row.effective<row.base?'基础 Lv.'+row.base:'牧场 Lv.'+row.effective)+'</small>';
      let values;
      if(row.drop) values='<td>'+qty(row.drop)+'</td><td>'+escape(row.drop.chance+'%')+'</td>';
      else {
        const text=row.effective<row.base?'基础 Lv.'+row.base:row.first>row.effective?'Lv.'+row.first+' 起':'本级无产出';
        values='<td colspan="2" class="rn-muted">'+text+'</td>';
      }
      return '<tr><td><a class="rn-row-name" href="'+route(other,row.id)+'" title="'+(byPal?'反查这种材料的放牧帕鲁':'查看这只帕鲁的全部产物')+'">'+thumbnail(other,row.id)+'<span>'+escape(rowname)+meta+'</span></a></td>'+values+
        '<td><a class="rn-link" href="'+wiki(other,row.id)+'" aria-label="'+escape('查看'+rowname+'的'+label+'图鉴')+'">'+label+'</a></td></tr>';
    }).join('')+'</tbody></table>';
    el('hint').textContent='按帕鲁面板上的牧场工作适应性选择等级，包含浓缩和适应性手册带来的提升。每次产出抽取一种物品，数量为抽中后的个数。';
  }
  function detail() {
    const byPal=mode==='pal';
    if(!candidates(mode).includes(key)) {
      el('profile').innerHTML='<p class="rn-empty">当前没有这'+(byPal?'只帕鲁':'种物品')+'的放牧产出记录。</p>';
      root.querySelector('.rn-detailbar').hidden=true;
      el('results').replaceChildren();el('hint').textContent='';el('source').replaceChildren();
      return;
    }
    root.querySelector('.rn-detailbar').hidden=false;
    const p=byPal?data.pals[key]:null;
    const meta=byPal?'基础牧场 Lv.'+p.base+' · 满浓缩 Lv.'+p.condensed:suppliers.get(key).length+' 种帕鲁可放牧产出';
    el('profile').innerHTML='<div class="rn-profile">'+thumbnail(mode,key)+'<div><b>'+escape(name(mode,key))+'</b><small>'+meta+'</small></div><a class="rn-link" href="'+wiki(mode,key)+'">'+(byPal?'帕鲁图鉴':'物品图鉴')+'</a></div>';
    el('table-title').textContent=byPal?'放牧产物':'产出帕鲁';
    const control=el('level');
    control.replaceChildren(new Option(byPal?'基础 Lv.'+p.base:'各自基础等级','0'));
    for(let i=byPal?p.base:1;i<=10;i++) control.add(new Option('Lv.'+i,i));
    let level=saved[mode].level;
    if(byPal && level<p.base) level=0;
    control.value=String(level);
    el('source').innerHTML='游戏数据 1.0.5 · <a href="https://paldb.cc/cn/Farming" target="_blank" rel="noopener noreferrer">牧场适应性与产物名录</a>'+
      (p?' · <a href="'+escape(p.source)+'" target="_blank" rel="noopener noreferrer">'+escape(name(mode,key))+'逐级产出表</a>':'');
    table();
  }
  function show(id,direction) {
    mode=direction==='item'?'item':'pal';
    key=id||saved[mode].key;
    if(candidates(mode).includes(key)) {
      saved[mode].key=key;
      if(!score(mode,key,normalize(saved[mode].query))) saved[mode].query='';
    }
    const byPal=mode==='pal';
    el('pal-mode').href=route('pal',saved.pal.key);
    el('item-mode').href=route('item',saved.item.key);
    setCurrent(el('pal-mode'),byPal);setCurrent(el('item-mode'),!byPal);
    el('search-label').textContent=byPal?'查找帕鲁':'查找材料';
    el('search').placeholder=byPal?'中文名、英文名或图鉴编号':'中文名、英文名或物品 ID';
    el('search').value=saved[mode].query;
    list();detail();
  }
  el('summary').textContent='家畜牧场可放牧 '+producers.length+' 种帕鲁，产出 '+materials.length+' 种材料与物品。';
  el('search').addEventListener('input',()=>{saved[mode].query=el('search').value;list();});
  el('search').addEventListener('keydown',event=>{
    if(event.key==='Enter'&&!event.isComposing) el('list').querySelector('a')?.click();
  });
  el('level').addEventListener('change',()=>{saved[mode].level=Number(el('level').value);table();});
  return {show};
}

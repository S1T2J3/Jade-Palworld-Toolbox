function initAdventureTables({data,itemIcon,escape}) {
  const el=id=>document.getElementById('ad-'+id);
  const search=el('find'), group=el('group'), list=el('list'), detail=el('detail');
  const enc=encodeURIComponent;
  const itemName=id=>data.itemNames[id]||id;
  const norm=s=>String(s).normalize('NFKC').trim().toLowerCase();
  const questHref=id=>'#guide/quests/'+enc(id);
  const poolHref=(chapter,pool)=>'#guide/loot/'+enc(chapter)+(pool?'/'+enc(pool):'');
  const link=(url,label)=>'<a href="'+escape(url)+'">'+escape(label)+'</a>';
  const heading=(title,body)=>'<section><h3>'+escape(title)+'</h3>'+body+'</section>';
  const qs=new Map(data.quests.map(q=>[q.id,q])), chapters=new Map(data.chapters.map(c=>[c.id,c]));
  const old=new Map();
  for(const q of data.quests)for(const next of q.next){
    if(!old.has(next))old.set(next,[]);
    old.get(next).push(q);
  }
  const pools=data.chapters.flatMap(ch=>ch.pools.map(p=>({...p,chapter:ch.id,group:ch.name})));
  const routePool=new Map(pools.map(p=>[p.chapter+'/'+p.id,p]));
  for(const p of pools) p.terms=norm([p.id,p.name,p.group,...new Set(p.rows.flatMap(r=>[itemName(r[0]),r[0]]))].join(' '));
  const questTerms=new Map(data.quests.map(q=>[q.id,norm([q.name,q.id,q.code,q.group,q.summary,
    ...q.objectives.concat(q.rewards).map(v=>v.item?itemName(v.item)+' '+v.item:v.text)].join(' '))]));
  const allQuestGroups=[...new Set(data.quests.map(q=>q.group))];
  const state={quests:{search:'',group:''},loot:{search:'',group:''}};
  let view, active=null, onlyItem=null, results=[];
  function itemCard(id,sub) {
    const img=itemIcon(id);
    return '<a class="ad-reward" href="#items/'+enc(id)+'">'+(img?'<img alt="" loading="lazy" src="'+escape(img)+'">':'')+
      '<span><b>'+escape(itemName(id))+'</b><small>'+escape(sub)+'</small></span></a>';
  }
  function objective(v) {
    if(v.item)return link('#items/'+enc(v.item),itemName(v.item))+' ×'+escape(v.qty);
    if(v.point)return link('#map/point/'+enc(v.point),v.text);
    if(v.site)return link('#map/coords/'+v.site.map(enc).join('/'),v.text);
    if(v.pal)return link('#paldex/'+enc(v.pal),v.text);
    if(v.unit && data.unitRoutes?.[v.unit])return link(data.unitRoutes[v.unit],v.text);
    if(v.passive)return link('#details/passive/trait/'+enc(v.passive),v.text);
    return escape(v.text);
  }
  function renderQuest(q) {
    let content='<span class="ad-note">'+escape(q.group)+(q.repeatable?' · 可重复接取':'')+'</span><h2>'+escape(q.name)+'</h2>';
    if(q.summary)content+='<p>'+escape(q.summary)+'</p>';
    if(q.objectives.length)content+=heading('目标与地点','<ol class="ad-objectives">'+q.objectives.map(v=>'<li>'+objective(v)+'</li>').join('')+'</ol>');
    const rewards=q.rewards.filter(v=>v.item), other=q.rewards.filter(v=>!v.item);
    content+=heading('任务奖励',(other.length?'<p class="ad-note">'+other.map(v=>objective(v)).join(' · ')+'</p>':'')+
      (rewards.length?'<div class="ad-rewards">'+rewards.map(v=>itemCard(v.item,'×'+v.qty)).join('')+'</div>':'')+
      (!q.rewards.length?'<p class="ad-note">任务表没有列出固定奖励。</p>':''));
    for(const [name,links] of [['前序任务',old.get(q.id)||[]],['后续任务',q.next.map(id=>qs.get(id))]]) {
      if(links.length)content+=heading(name,'<div class="ad-row">'+links.map(v=>link(questHref(v.id),v.name)).join('')+'</div>');
    }
    return content;
  }
  function relatedPool(p) {
    const mapId=p.id.toLowerCase().includes('worldtree')?'worldtree':'palpagos';
    const mapKind={EnemyCampLoots:'enemy_camp',SkillFruitLoots:'skill_fruit',DungeonsLoots:'dungeon',ElementalChestsLoots:'chest',OilrigLoots:'chest',FishingLoots:'fishing'}[p.chapter];
    if(p.chapter==='PalExpeditionStationLoots')return link('#work/expeditions/'+enc(p.id),'查看远征条件');
    if(p.chapter==='AncientRelicRecyclerLoots')return link('#work/relic/'+enc(p.id.replace('AncientRelicRecycler_','')),'查看遗物转换产量');
    if(mapKind){
      const suffix=p.chapter==='OilrigLoots'?'/'+enc('油田'):'';
      const label={enemy_camp:'敌人据点',skill_fruit:'技能果实树',dungeon:'地下城入口',chest:'宝箱',fishing:'钓鱼点'}[mapKind];
      return link('#map/category/'+mapKind+'/'+mapId+suffix,'地图 · '+(mapId==='worldtree'?'世界树':'帕洛斯群岛')+label);
    }
    return '';
  }
  function rowCard(r) {
    const n=r[1]===r[2]?'×'+r[1]:'×'+r[1]+'–'+r[2];
    const pct=r[3]<.001?Number(r[3].toPrecision(2)):Number(r[3].toFixed(3));
    return itemCard(r[0],n+' · '+pct+'%'+(r[5]>1?' · 数量间隔 '+r[5]:''));
  }
  function renderPool(p) {
    let content='<span class="ad-note">'+escape(p.group)+' · '+escape(p.id)+'</span><h2>'+escape(p.name)+'</h2>';
    const related=relatedPool(p);
    if(related) content+='<div class="ad-row">'+related+'</div>';
    const rows=onlyItem?p.rows.filter(r=>r[0]===onlyItem):p.rows;
    const note=p.slotted?'百分比含该栏位的触发率。同一物品可能出现在多个栏位，每行分别显示。':
      '此奖池使用发布表的数量与单项获得率，未标出抽取栏位。';
    content+='<p class="ad-note">'+note+'</p>';
    if(onlyItem)content+='<div class="ad-row">'+link(poolHref(p.chapter,p.id),'查看完整奖池')+'</div>';
    if(!p.slotted)return content+heading('可能获得的物品','<div class="ad-rewards">'+rows.map(rowCard).join('')+'</div>');
    const bySlot=new Map();
    for(const row of rows){if(!bySlot.has(row[4]))bySlot.set(row[4],[]);bySlot.get(row[4]).push(row);}
    for(const [slot,entries] of [...bySlot].sort((a,b)=>a[0]-b[0])) {
      content+=heading('抽取栏位 '+slot,'<div class="ad-rewards">'+entries.map(rowCard).join('')+'</div>');
    }
    return content;
  }
  function href(record) {
    if(view==='quests')return questHref(record.id);
    return poolHref(record.chapter,record.id);
  }
  function refresh(preferred,missing=false) {
    const terms=norm(search.value).split(/\s+/).filter(Boolean);
    if(view==='quests'){
      results=data.quests.filter(q=>(!group.value||q.group===group.value)&&terms.every(t=>questTerms.get(q.id).includes(t)));
    }else{
      results=pools.filter(p=>(!group.value||p.chapter===group.value)&&(!onlyItem||p.rows.some(r=>r[0]===onlyItem))&&terms.every(t=>p.terms.includes(t)));
    }
    active=missing?null:results.includes(preferred)?preferred:results[0]||null;
    el('count').textContent=(onlyItem?itemName(onlyItem)+' · ':'')+results.length+(view==='quests'?' 项任务':' 个奖池');
    list.innerHTML=results.map(row=>'<a class="ad-choice" href="'+escape(href(row))+'"'+(row===active?' aria-current="true"':'')+'>'+escape(row.name)+
      '<small>'+escape(row.group)+(view==='loot'?' · '+row.rows.length+' 行':'')+'</small></a>').join('');
    detail.innerHTML=active?(view==='quests'?renderQuest(active):renderPool(active)):'<p class="ad-note">没有找到对应条目，可调整搜索条件。</p>';
  }
  function filtersChanged() {
    state[view]={group:group.value,search:search.value};
    onlyItem=null;
    refresh(active);
    history.replaceState(history.state,'',active?href(active):'#guide/'+view);
  }
  search.addEventListener('input',filtersChanged);group.addEventListener('change',filtersChanged);
  list.addEventListener('click',e=>{
    const anchor=e.target.closest('a');
    if(!anchor||e.ctrlKey||e.metaKey||e.shiftKey||e.altKey)return;
    if(onlyItem){
      e.preventDefault();
      const index=Array.from(list.children).indexOf(anchor);
      refresh(results[index]);
    }
    if(matchMedia('(max-width:800px)').matches) requestAnimationFrame(()=>{detail.scrollIntoView({block:'start'});detail.focus({preventScroll:true});});
  });
  return {show(next,id,arg,extra) {
    if(view!==next){
      view=next;active=null;onlyItem=null;
      const isQuest=view==='quests';
      el('title').textContent=isQuest?'任务详情':'探索奖励表';
      search.placeholder=isQuest?'名称、目标或奖励物品':'区域、奖池或物品';
      group.replaceChildren(new Option('全部分类',''));
      for(const [id,label] of (isQuest?allQuestGroups.map(s=>[s,s]):data.chapters.map(c=>[c.id,c.name])))group.add(new Option(label,id));
      group.value=state[view].group;search.value=state[view].search;
    }
    onlyItem=view==='loot'&&arg==='item'?extra||null:null;
    let requested=null, missing=false;
    if(view==='quests'&&id){
      requested=qs.get(id);missing=!requested;
      if(requested&&(!results.includes(requested)||group.value&&group.value!==requested.group)){search.value='';group.value='';}
    }else if(view==='loot'&&id){
      const chapter=chapters.get(id);
      group.value=id;
      if(chapter&&arg&&arg!=='item'){requested=routePool.get(id+'/'+arg);missing=!requested;}
      if(!requested||!results.includes(requested))search.value='';
      if(!chapter)missing=true;
    }
    refresh(requested||active,missing);
  }};
}

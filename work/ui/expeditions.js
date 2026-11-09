function initExpeditions({data,items,itemIcon}) {
  const el=k=>document.getElementById('ex-'+k),stages=data.stages;
  const byId=new Map(stages.map(s=>[s.id,s])),catalog=new Map(items.map(i=>[i.id,i]));
  const esc=value=>String(value??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
  const norm=s=>s.normalize('NFKC').trim().toLowerCase();
  const route=id=>'#work/expeditions/'+encodeURIComponent(id);
  const name=id=>catalog.get(id)?.name||id;
  const tier=s=>s.hard?'高难':'普通';
  const termById=new Map(stages.map(s=>[s.id,norm([s.name,s.region,tier(s),s.id,...s.rewards.map(r=>name(r[0]))].join(' '))]));
  const search=el('search'),mode=el('tier'),element=el('element'),list=el('list'),detail=el('detail');
  el('summary').textContent=stages.length+' 个目的地 · 按派遣条件和奖励查找远征。';
  for(const e of new Set(stages.map(s=>s.element?.[0]).filter(Boolean))) element.add(new Option(e+'属性',e));
  let active=null,shown=[];
  function rewards(s){
    // Keep each loot row. Identical rows from separate reward slots may repeat.
    const grouped=new Map();
    for(const [id,lo,hi,p] of s.rewards){
      if(!grouped.has(id))grouped.set(id,new Map());
      const slot='×'+lo+(hi!==lo?'–'+hi:'')+' · '+p+'%';
      const slots=grouped.get(id);slots.set(slot,(slots.get(slot)||0)+1);
    }
    return [...grouped].map(([id,slots])=>{
      const url=itemIcon(id),lines=[...slots].map(([slot,n])=>'<small>'+slot+(n>1?'（'+n+' 个栏位）':'')+'</small>').join('');
      return '<a class="ex-reward" href="#items/'+encodeURIComponent(id)+'">'+(url?'<img alt="" loading="lazy" src="'+esc(url)+'">':'')+
        '<span><b>'+esc(name(id))+'</b>'+lines+'</span></a>';
    }).join('');
  }
  function renderDetail(s) {
    if(!s){detail.hidden=true;detail.replaceChildren();return;}
    detail.hidden=false;
    const stats=[['基础时长',s.minutes+' 分钟'],['推荐战斗力',s.power.toLocaleString('zh-CN')],['派遣属性',s.element?s.element[0]+'属性 ×'+s.element[1]:'无属性限制']];
    detail.innerHTML='<div class="ex-kicker">'+esc(s.region)+' · '+tier(s)+'</div><h2>'+esc(s.name)+'</h2><dl class="ex-stats">'+
      stats.map(([k,v])=>'<div><dt>'+k+'</dt><dd>'+esc(v)+'</dd></div>').join('')+'</dl><p>'+esc(s.unlock)+'</p>'+
      '<div class="ex-links"><a href="#map/point/'+encodeURIComponent(s.point)+'">地图定位 · '+esc(s.tower)+'</a><a href="#details/stats">计算帕鲁远征战力</a><a href="#work/facilities/Pal_Expedition_Station">远征所建造材料</a></div>'+
      '<h3>可能获得的物品</h3><p class="ex-note">数量与概率对应奖励表的单个栏位；同一物品可以出现在多个栏位。</p><div class="ex-rewards">'+rewards(s)+'</div>';
  }
  function refresh(requested){
    const query=norm(search.value);
    shown=stages.filter(s=>(!mode.value||mode.value===(s.hard?'hard':'normal'))&&(!element.value||element.value===(s.element?.[0]||'any'))&&termById.get(s.id).includes(query));
    active=shown.find(s=>s.id===requested)||shown[0]||null;
    list.innerHTML=shown.map(s=>'<a href="'+route(s.id)+'" aria-current="'+(s===active)+'"><span>'+esc(s.name)+'</span><small>'+tier(s)+' · '+s.minutes+' 分</small></a>').join('')||'<p class="ex-note">没有符合条件的远征。</p>';
    renderDetail(active);
  }
  for(const control of [search,mode,element]) control.addEventListener(control===search?'input':'change',()=>{
    refresh(active?.id);history.replaceState(history.state,'',active?route(active.id):'#work/expeditions');
  });
  list.addEventListener('click',e=>{
    if(e.target.closest('a')&&!e.ctrlKey&&!e.metaKey&&!e.altKey&&!e.shiftKey&&window.matchMedia('(max-width:760px)').matches)
      requestAnimationFrame(()=>{detail.scrollIntoView({block:'start'});detail.focus({preventScroll:true});});
  });
  return {show(id){
    if(id&&byId.has(id)&&!shown.some(s=>s.id===id)){search.value='';mode.value='';element.value='';}
    refresh(id||active?.id);
  }};
}

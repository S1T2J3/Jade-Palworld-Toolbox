function initPalDetails({data,species,spawnData,avatar,dex,typeSet,normalize,matchScore,escape,icon,itemIcon,surgeryBook}) {
  const el=id=>document.getElementById('pd-'+id);
  const tabs={
    active:['主动技能','查看技能威力、冷却时间和帕鲁习得等级。'],
    passive:['被动技能','查询词条效果，或选择帕鲁查看其固定被动。'],
    work:['工作适应性','按工作种类查看基础等级排行，同级按图鉴顺序排列。'],
    fruits:['技能果实','查看每种果实教授的主动技能，点击果实可查询获取方式。'],
    surgery:['帕鲁手术台','查询可用手术、植入体和每次手术的费用。'],
    stats:['属性值计算','选择帕鲁，计算培养后的属性和远征战力。'],
    capture:['捕捉概率','按目标等级、剩余生命和捕捉装备比较各类帕鲁球。']
  };
  const types={neutral:'无', fire:'火', water:'水', electric:'雷', grass:'草', ice:'冰', ground:'地', dark:'暗', dragon:'龙'};
  const byKey=new Map(species.map(p=>[p.key,p]));
  const profiles=spawnData.profiles, jobs=Object.keys(data.workIcons);
  const passives=Object.entries(data.passives).sort((a,b)=>comparePassiveTraits(a[1],b[1]));
  const moves=Object.entries(data.moves).sort((a,b)=>Object.keys(types).indexOf(a[1].type)-Object.keys(types).indexOf(b[1].type)||a[1].power-b[1].power);
  const fixedBy=new Map(), learnedBy=new Map();
  for (const pal of species) {
    const profile=data.profiles[pal.key];
    if (!profile) continue;
    for (const [level,id] of profile.levels) {
      if (!learnedBy.has(id)) learnedBy.set(id,[]);
      learnedBy.get(id).push([pal,level]);
    }
    for (const key of profile.fixed) {
      if (!fixedBy.has(key)) fixedBy.set(key,[]);
      fixedBy.get(key).push(pal);
    }
  }
  const state=Object.fromEntries(Object.keys(tabs).map(k=>[k,{raw:'',query:'',scope:''}]));
  let tab='active', current=null, highlighted='', job='手工作业', cursor=-1, matched=[];
  const route=(section,key)=>(section==='work'?'#work/suitability':'#details/'+section)+(key?'/'+encodeURIComponent(key):'');
  const entryHref=(section,id,pal)=>route(section,pal?.key||(section==='active'?'move':'trait'))+'/'+encodeURIComponent(id);
  const itemHref=key=>'#items/'+encodeURIComponent(key);
  const itemLink=(key,label)=>`<a class="pd-link" href="${itemHref(key)}">${escape(label)}</a>`;
  const img=(src,cls)=>src?`<img class="${cls}" src="${src}" alt="" loading="lazy" decoding="async">`:'';
  const typeTag=t=>`<span class="pd-type pd-${t}">${icon(t)}${types[t]}</span>`;
  const rankTag=rank=>`<span class="pd-rank pd-rank-${rank<0?'minus':rank}">阶级 ${rank}</span>`;
  const workList=pal=>profiles[pal.key]?.work || [];
  const matchText=(...fields)=>!state[tab].query || fields.some(f=>normalize(f||'').includes(state[tab].query));
  const count=(n,unit)=>{el('count').textContent=n+' '+unit;};
  const empty=text=>`<p class="pd-empty">${escape(text)}</p>`;
  function show(tabName,speciesKey,entryKey) {
    const last=current;
    highlighted='';
    tab=Object.hasOwn(tabs,tabName)?tabName:'active';
    const calculator=tab==='stats'||tab==='capture';
    current=byKey.get(speciesKey) || (calculator?species[0]:null);
    const entries=tab==='active'?data.moves:tab==='passive'||tab==='surgery'?data.passives:null;
    if(entryKey&&entries&&Object.hasOwn(entries,entryKey)&&(current||speciesKey===(tab==='active'?'move':'trait'))&&(tab!=='surgery'||entries[entryKey].surgery)) highlighted=entryKey;
    if (highlighted || (current!==last && (tab==='active'||tab==='passive'))) {
      state[tab]={raw:'',query:'',scope:''};
    }
    if (tab==='work' && current && current!==last && workList(current).length && !workList(current).some(([j])=>j===job)) job=workList(current)[0][0];
    el('intro').textContent=tabs[tab][1];
    el('title').textContent=tabs[tab][0];
    const palMode=calculator||['active','passive','work'].includes(tab);
    el('picker').hidden=!palMode;
    el('pal-search').value='';
    closePicker();
    const context=el('context');
    context.hidden=!current||!palMode;
    if (context.hidden) context.replaceChildren();
    else {
      const partner=profiles[current.key]?.partner;
      context.innerHTML=`<div class="pd-palhead">${avatar(current)}<div><b>${escape(current.zh)}</b><small>${escape(dex(current))} · ${escape(current.en)}</small></div></div><div class="pd-identity">${typeSet(current)}${partner?`<a class="pd-link" href="#details/partner/${encodeURIComponent(current.key)}">伙伴技能 · ${escape(partner)}</a>`:''}</div><div class="pd-actions"><a class="pd-link" href="#spawns/${encodeURIComponent(current.key)}">查看刷新位置</a><a href="${route(tab)}" class="pd-clear">查看全部帕鲁</a></div>`;
    }
    if(calculator) context.querySelector('.pd-clear')?.remove();
    el('list-page').hidden=calculator;
    el('sources').hidden=calculator;
    document.getElementById('pal-stat-calc').hidden=tab!=='stats';
    document.getElementById('pal-capture-calc').hidden=tab!=='capture';
    if(calculator) return;
    el('subtitle').textContent=({active:'技能列表',passive:current?'固定被动词条':'词条列表',fruits:'果实列表',surgery:'手术项目',work:'适应性排行'})[tab];
    let options=[['','全部属性'],...Object.entries(types).map(([k,n])=>[k,n+'属性'])];
    if (tab==='passive') options=[['','全部词条'],['random','可随机出现'],['fixed','有固定携带种'],['surgery','手术可用'],['negative','负阶级']];
    if (tab==='work') options=[['','全部属性'],['dark','仅暗属性']];
    if (tab==='surgery') options=[['','全部手术'],['owned','当前可用'],['needed','尚未持有'],['coins','直接付费'],['unlock','解锁植入体'],['consume','耗材植入体']];
    el('scope').innerHTML=options.map(([k,n])=>`<option value="${k}">${n}</option>`).join('');
    el('scope').value=state[tab].scope;
    el('filter').value=state[tab].raw;
    el('filter').placeholder={active:'搜索技能名或异常状态',passive:'搜索词条名或效果',work:'搜索帕鲁名称或编号',fruits:'搜索果实或技能',surgery:'搜索词条、效果或植入体'}[tab];
    el('work-tabs').hidden=tab!=='work';
    render();
  }
  function renderActive(scope) {
    const pool=current?(data.profiles[current.key]?.levels||[]).map(([lv,id])=>[id,data.moves[id],lv]):moves;
    const rows=pool.filter(([id,m])=>(!highlighted||id===highlighted)&&(!scope||m.type===scope)&&matchText(m.zh,m.en,...(m.effects||[]).map(([name])=>name)));
    count(rows.length,'项');
    if (!rows.length) return empty(current?.key==='WorldTreeDragon'?'枯星龙为塔主帕鲁，普通形态没有升级习得记录。':'没有匹配的主动技能。');
    return `<div class="pd-grid">${rows.map(([id,m,lv])=>`<details class="pd-card pd-move" data-move="${escape(id)}"><summary><div class="pd-cardtop">${lv?`<span class="pd-level">Lv. ${lv}</span>`:''}<b>${escape(m.zh)}</b>${typeTag(m.type)}</div><div class="pd-skillmeta"><span>威力 <strong>${m.power}</strong></span><span>冷却 <strong>${m.cooldown} 秒</strong></span><span class="pd-expand" aria-hidden="true">⌄</span></div></summary><div class="pd-expanded"></div></details>`).join('')}</div>`;
  }
  function renderPassive(scope) {
    const fixed=current?data.profiles[current.key]?.fixed||[]:null;
    const rows=passives.filter(([id,p])=>(!highlighted||id===highlighted)&&(!fixed||fixed.includes(id))&&(!scope||(scope==='fixed'?fixedBy.has(id):scope==='negative'?p.rank<0:p[scope]))&&matchText(p.zh,p.en,p.effect));
    count(rows.length,'条');
    const note=current?'<p class="pd-note">这里列出该种帕鲁的固定词条。个体还可能通过随机生成、遗传或手术获得其他词条。</p>':'';
    if (!rows.length) return note+empty(current&&!fixed.length?'该种帕鲁没有已记录的固定被动。':'没有匹配的词条。');
    return note+`<div class="pd-grid">${rows.map(([id,p])=>`<details class="pd-card pd-passive" data-passive="${escape(id)}"><summary><div class="pd-cardtop"><b>${passiveRankIcon(p.rank)}${escape(p.zh)}</b>${rankTag(p.rank)}</div><p class="pd-effect">${escape(p.effect)}</p></summary><div class="pd-expanded"></div></details>`).join('')}</div>`;
  }
  function renderFruits(scope) {
    const fruits=moves.filter(([,m])=>m.fruit&&(!scope||scope===m.type)&&matchText(m.zh,m.en,m.fruit));
    count(fruits.length,'种');
    if (!fruits.length) return empty('没有匹配的技能果实。');
    return `<div class="pd-grid">${fruits.map(([id,m])=>`<a href="${itemHref('SkillCard_'+id)}" class="pd-card pd-fruit">${img(data.fruitIcons[m.type],'pd-fruit-art')}<div><b>${escape(m.fruit)}</b><small>${escape(m.zh)} · 威力 ${m.power} · ${m.cooldown} 秒</small></div><span class="pd-next" aria-hidden="true">›</span></a>`).join('')}</div>`;
  }
  function renderSurgery(scope) {
    const available=key=>surgeryBook?.has(key)??data.passives[key].surgery.mode==='coins';
    const rows=passives.filter(([key,p])=>(!highlighted||key===highlighted)&&p.surgery&&(!scope || p.surgery.mode===scope ||
      (scope==='owned'&&available(key)) || (scope==='needed'&&!available(key)))&&matchText(p.zh,p.en,p.effect,p.surgery.name,p.surgery.source));
    count(rows.length,'种词条手术');
    const reverseArt=itemIcon?.('PalGenderReverse');
    const overview=`<div class="pd-table-info"><div class="pd-table-unit">${img(data.tableIcon,'pd-table-art')}<div><b>建造手术台 <small>科技 Lv. 38</small></b><p>${itemLink('IronIngot','精炼金属锭 ×30')} · ${itemLink('Cloth2','优质的布 ×20')} · ${itemLink('Corrosive_Solvent','腐蚀溶剂 ×10')}</p></div></div><div class="pd-table-unit">${img(reverseArt,'pd-reverse-art')}<div><b>更改性别</b><p>在手术台使用${itemLink('PalGenderReverse','帕鲁性转药')}。</p></div></div></div>`;
    const recordNote=`<div class="pd-operation-note"><p>直接付费的词条随手术台开放。勾选已解锁的植入体或手中有余量的耗材，即可在目标配种中使用。</p><a class="pd-link" href="#planner">前往目标配种</a></div>`;
    const saveNote=surgeryBook&&!surgeryBook.isPersistent()?'<p class="pd-note" role="status">浏览器无法保存，当前标记只在本次打开期间有效。</p>':'';
    if (!rows.length) return overview+recordNote+saveNote+empty('没有匹配的手术。');
    return overview+recordNote+saveNote+`<div class="pd-grid">${rows.map(([key,p])=>{
      const s=p.surgery, fee=s.cost.toLocaleString('zh-CN')+' 金币';
      const condition=s.mode==='consume'?'每次消耗 1 个':s.mode==='unlock'?s.source+'解锁 · 每次 '+fee:'每次 '+fee;
      const text=s.mode==='consume'?'持有耗材':s.mode==='unlock'?'已获取植入体':'手术台自带';
      const mark=`<label class="pd-ownership"><input type="checkbox" data-pd-operation="${escape(key)}" aria-label="${escape(p.zh+' · '+text)}" ${available(key)?'checked':''} ${s.mode==='coins'?'disabled':''}>${text}</label>`;
      return `<article class="pd-card pd-surgery"><div class="pd-cardtop"><b><a class="pd-link" href="${entryHref('passive',key)}">${passiveRankIcon(p.rank)}${escape(p.zh)}</a></b>${rankTag(p.rank)}</div><p class="pd-effect">${escape(p.effect)}</p><div class="pd-surgery-foot"><span>${condition}</span>${s.item?itemLink(s.item,s.name):''}</div>${mark}</article>`;
    }).join('')}</div>`;
  }
  function renderWork(scope) {
    const own=new Map(current?workList(current):[]);
    el('work-tabs').innerHTML=jobs.map(name=>`<button type="button" data-job="${name}" aria-pressed="${job===name}">${img(data.workIcons[name],'pd-job-art')}${name}${own.has(name)?`<small>Lv.${own.get(name)}</small>`:''}</button>`).join('');
    const rows=[];
    species.forEach((pal,i)=>{
      const lv=workList(pal).find(([j])=>j===job)?.[1];
      if (lv && (!scope||pal.types.includes(scope)) && (!state.work.query||matchScore(i,state.work.query))) rows.push([pal,lv]);
    });
    rows.sort((a,b)=>b[1]-a[1]);
    count(rows.length,'种帕鲁');
    const hint='<p class="pd-note">'+(current?escape(current.zh)+(own.has(job)?'的'+job+'基础等级为 Lv. '+own.get(job)+'。':'没有'+job+'适应性。')+' ':'')+
      '<a class="pd-link" href="#work/efficiency/'+(current?encodeURIComponent(current.key)+'/':'')+encodeURIComponent(job)+'">'+(current?'计算这只帕鲁的'+escape(job)+'效率':'计算'+escape(job)+'效率')+'</a></p>';
    if (!rows.length) return hint+empty('该工作下没有匹配的帕鲁。');
    return hint+[...new Set(rows.map(([,lv])=>lv))].map(lv=>{
      const pals=rows.filter(([,level])=>level===lv).map(([pal])=>pal);
      return `<section class="pd-work-group"><h3><span>Lv. ${lv}</span>${job}<small>${pals.length} 种</small></h3><div class="pd-work-grid">${pals.map(p=>`<a class="pd-worker${current?.key===p.key?' pd-selected':''}" href="#paldex/${encodeURIComponent(p.key)}">${avatar(p)}<span><b>${escape(p.zh)}</b><small>${escape(dex(p))}</small></span>${p.types.includes('dark')?typeTag('dark'):''}</a>`).join('')}</div></section>`;
    }).join('');
  }
  const renderers={active:renderActive,passive:renderPassive,fruits:renderFruits,work:renderWork,surgery:renderSurgery};
  function render() {
    if(!renderers[tab])return;
    const all=highlighted?`<p class="pd-note"><a class="pd-link" href="${route(tab,current?.key)}">查看${current?escape(current.zh)+'的':''}全部${tabs[tab][0]}</a></p>`:'';
    const results=el('results');
    results.innerHTML=all+renderers[tab](state[tab].scope);
    const card=highlighted&&results.querySelector('details.pd-card');
    if(card){card.open=true;expandCard(card);}
  }
  function releaseFocus() {
    if(!highlighted)return;
    highlighted='';
    history.replaceState(history.state,'',route(tab,current?.key));
  }
  function expandCard(detail) {
    const body=detail.querySelector('.pd-expanded');
    if (!detail.open||!body||body.hasChildNodes()) return;
    const id=detail.dataset.move;
    if (id) {
      const m=data.moves[id], pals=learnedBy.get(id)||[];
      const statusIds={'浸湿':'wet','点燃':'burn','缠绕':'ivy','冻结':'freeze','感电':'shock','泥泞':'mud','眩晕':'stun','中毒':'poison','黑暗':'blind'};
      const effects=(m.effects||[]).map(([name,value])=>(statusIds[name]?`<a class="pd-link" href="#guide/status/${statusIds[name]}">${escape(name)}</a>`:escape(name))+' '+escape(value)).join('、');
      body.innerHTML=`<p>${escape(m.en)}</p><p>${m.inherit?'可遗传':'不可随机遗传'}${effects?' · 积蓄效果 '+effects:''}</p>${m.fruit?`<p>${itemLink('SkillCard_'+id,m.fruit)}</p>`:''}<b>升级习得 · ${pals.length} 种</b>`+(pals.length?`<div class="pd-pal-chips">${pals.map(([p,lv])=>`<a href="${entryHref('active',id,p)}">${escape(p.zh)} <small>Lv.${lv}</small></a>`).join('')}</div>`:'<p>此技能没有普通帕鲁的升级习得记录。</p>');
    } else {
      const key=detail.dataset.passive, p=data.passives[key], pals=fixedBy.get(key)||[];
      body.innerHTML=`<p>${escape(p.en)}</p><p>${p.random?'可随机出现':'不在普通随机词条池'}${p.surgery?` · <a class="pd-link" href="${entryHref('surgery',key)}">可通过手术添加</a>`:''}</p>${p.surgery?.item?`<p>${itemLink(p.surgery.item,p.surgery.name)}</p>`:''}`+(pals.length?`<b>固定携带 · ${pals.length} 种</b><div class="pd-pal-chips">${pals.map(pal=>`<a href="${entryHref('passive',key,pal)}">${escape(pal.zh)}</a>`).join('')}</div>`:'');
    }
  }
  function closePicker() {
    el('pal-options').hidden=true;
    el('pal-options').replaceChildren();
    el('pal-search').setAttribute('aria-expanded','false');
    el('pal-search').removeAttribute('aria-activedescendant');
    cursor=-1; matched=[];
  }
  function drawPicker() {
    const q=normalize(el('pal-search').value);
    matched=species.map((p,i)=>[p,matchScore(i,q)]).filter(([,s])=>s>0).sort((a,b)=>b[1]-a[1]).map(([p])=>p);
    cursor=-1;
    el('pal-options').hidden=false;
    el('pal-search').setAttribute('aria-expanded','true');
    el('pal-search').removeAttribute('aria-activedescendant');
    el('pal-options').innerHTML=matched.length?matched.map((p,i)=>`<div id="pd-option-${i}" role="option" aria-selected="false" data-key="${escape(p.key)}"><span>${escape(p.zh)}</span><small>${escape(dex(p))} · ${escape(p.en)}</small></div>`).join(''):empty('没有匹配的帕鲁。');
  }
  function pick(key) {closePicker();el('pal-search').value='';location.hash=route(tab,key);}
  el('pal-search').addEventListener('focus',drawPicker);
  el('pal-search').addEventListener('input',drawPicker);
  el('pal-search').addEventListener('keydown',event=>{
    if (event.key==='Escape') {closePicker();return;}
    if (event.isComposing) return;
    if (event.key==='Enter'&&matched.length) {event.preventDefault();pick(matched[Math.max(0,cursor)].key);return;}
    if (!['ArrowDown','ArrowUp'].includes(event.key)) return;
    event.preventDefault();
    if (el('pal-options').hidden) drawPicker();
    if (!matched.length) return;
    el('pal-options').children[cursor]?.setAttribute('aria-selected','false');
    cursor=Math.max(0,Math.min(matched.length-1,cursor+(event.key==='ArrowDown'?1:-1)));
    const option=el('pal-options').children[cursor];
    option.setAttribute('aria-selected','true');
    el('pal-search').setAttribute('aria-activedescendant',option.id);
    option.scrollIntoView({block:'nearest'});
  });
  el('pal-options').addEventListener('mousedown',e=>e.preventDefault());
  el('pal-options').addEventListener('click',e=>{const key=e.target.closest('[data-key]')?.dataset.key;if(key)pick(key);});
  el('pal-search').addEventListener('blur',closePicker);
  el('filter').addEventListener('input',()=>{releaseFocus();state[tab].raw=el('filter').value;state[tab].query=normalize(state[tab].raw);render();});
  el('scope').addEventListener('change',()=>{releaseFocus();state[tab].scope=el('scope').value;render();});
  el('work-tabs').addEventListener('click',e=>{
    const button=e.target.closest('[data-job]');
    if (button) {job=button.dataset.job;render();el('work-tabs').querySelector('[aria-pressed="true"]').focus({preventScroll:true});}
  });
  el('results').addEventListener('toggle',e=>{if(e.target.tagName==='DETAILS')expandCard(e.target);},true);
  el('results').addEventListener('change',event=>{
    const key=event.target.dataset.pdOperation;
    if (key) surgeryBook?.set(key,event.target.checked);
  });
  surgeryBook?.subscribe(()=>{
    if (tab!=='surgery') return;
    const focused=document.activeElement?.dataset?.pdOperation;
    render();
    if (focused) (el('results').querySelector('[data-pd-operation="'+CSS.escape(focused)+'"]')||el('scope')).focus({preventScroll:true});
  });
  el('sources').innerHTML='游戏数据与图像来源 · '+data.sources.map(([name,url])=>`<a href="${escape(url)}" target="_blank" rel="noopener noreferrer">${escape(name)}</a>`).join(' · ');
  return {show,refresh:render,deactivate:closePicker};
}

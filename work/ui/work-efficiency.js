function calculateWorkEfficiency(data,job,level,keys) {
  const curve=data.levels[job];
  if(!curve||!Number.isInteger(level)||level<0||level>curve.length) throw new RangeError('工作种类或等级无效');
  const selected=[...new Set(keys)];
  if(selected.length>4) throw new RangeError('最多选择四个词条');
  let work=0,ranch=0,movement=0;
  for(const key of selected) {
    const bonus=Object.hasOwn(data.modifiers,key)?data.modifiers[key]:{};
    work+=bonus.work||0;ranch+=bonus.ranch||0;movement+=bonus.movement||0;
  }
  const rank=level?Math.min(curve.length,level+(job==='牧场'?ranch:0)):0;
  const base=rank?curve[rank-1]:0;
  const production=job!=='搬运'&&job!=='牧场';
  const amount=production?Math.max(0,base*(100+work)/100):base;
  return {rank,base,work,ranch,movement,amount};
}

function initWorkEfficiency({species,traits,profiles,inventory,escape,data}) {
  const el=key=>document.getElementById('we-'+key);
  const root=document.getElementById('work-efficiency-view');
  const jobs=Object.keys(data.levels),defaults=new Map(species.map(p=>[p.key,p]));
  const byTrait=new Map(traits.map(t=>[t.key,t]));
  const orderedTraits=[...traits].sort(comparePassiveTraits);
  const norm=text=>String(text??'').normalize('NFKC').trim().toLowerCase();
  const format=new Intl.NumberFormat('zh-CN',{maximumFractionDigits:2,useGrouping:false});
  const number=n=>format.format(n);
  const signed=n=>(n>0?'+':'')+number(n);
  let origin='manual',job='生火',level=1,selected=[],showAll=false;
  let sourceId='',palKey='',sourceRows=[],currentRecord=null;
  function workOf(key){return profiles[key]?.work||[];}
  function traitName(key){return byTrait.get(key)?.zh||String(key);}
  function detail(key) {
    const m=data.modifiers[key];
    if(!m) return '';
    if(job==='搬运') return m.movement?signed(m.movement)+'% 移速':'';
    if(job==='牧场') return m.ranch?signed(m.ranch)+' 级':'';
    return m.work?signed(m.work)+'%':'';
  }
  function note(text) {el('feedback').textContent=text;}
  for(const name of jobs) el('job').add(new Option(name,name));
  for(let n=0;n<=10;n++) el('level').add(new Option(n?'Lv. '+n:'无此适应性',String(n)));
  function updateTraits() {
    el('used').textContent=selected.length+' / 4';
    el('picked').innerHTML=selected.map(key=>'<button type="button" data-we-remove="'+escape(key)+'" aria-label="移除'+escape(traitName(key))+'">'+passiveRankIcon(byTrait.get(key)?.rank)+escape(traitName(key))+' ×</button>').join('');
    el('relevant').setAttribute('aria-pressed',String(!showAll));el('all').setAttribute('aria-pressed',String(showAll));
    const q=norm(el('query').value),visible=orderedTraits.filter(t=>{
      if(q) return [t.zh,t.en,t.key].some(s=>norm(s).includes(q));
      return showAll||Boolean(detail(t.key));
    });
    el('options').innerHTML=visible.map(t=>{
      const active=selected.includes(t.key),effect=detail(t.key);
      return '<button type="button" data-we-trait="'+escape(t.key)+'" aria-pressed="'+active+'"'+(!active&&selected.length===4?' disabled':'')+'>'+passiveRankIcon(t.rank)+escape(t.zh)+(effect?'<small>'+escape(effect)+'</small>':'')+'</button>';
    }).join('');
    if(q&&!visible.length) el('options').textContent='没有匹配的词条。';
  }
  function updateResult() {
    el('job').value=job;el('level').value=String(level);
    const work=workOf(palKey),natural=work.find(pair=>pair[0]===job)?.[1]||0;
    const known=defaults.has(palKey);
    // An occupation without natural aptitude cannot be gained by assigning a rank.
    el('level').disabled=origin!=='manual'&&(!sourceId||(known&&!natural));
    el('level-note').textContent=origin==='manual'?'可填写浓缩、研习或据点支援后的等级；牧场词条会在计算时加上。':
      !sourceId?'请先选择帕鲁。':!known?'图鉴暂无此物种的工作资料，请按游戏内等级填写。':
      natural?'图鉴基础为 Lv. '+natural+'，可改成该个体经浓缩、研习或据点支援后的等级。':'该物种没有'+job+'适应性。';
    const r=calculateWorkEfficiency(data,job,level,selected);
    const carry=job==='搬运',farm=job==='牧场';
    el('result-title').textContent=carry?'单趟搬运上限（件）':farm?'牧场等级基础参数':'标准工作量';
    el('value').textContent=origin!=='manual'&&!sourceId?'—':number(r.amount);
    el('rank').textContent=r.rank?'Lv. '+r.rank:'—';
    el('base').textContent=number(r.base);
    el('mod-label').textContent=carry?'自身移动速度词条':farm?'牧场等级词条':'自身工作速度词条';
    el('mod').textContent=carry?signed(r.movement)+'%':farm?signed(r.ranch)+' 级':signed(r.work)+'%';
    const context=origin!=='manual'&&!sourceId?'等待选择帕鲁':!r.rank?'无法从事这项工作':carry?'Lv. '+r.rank+' · 每趟最多 '+r.base+' 件':farm?'牧场 Lv. '+level+(r.ranch?' + '+r.ranch+' 级（上限 10）':'')+' → Lv. '+r.rank:
      number(r.base)+' × '+number(Math.max(0,(100+r.work)/100))+' = '+number(r.amount);
    el('formula').textContent=context;
    el('scope').textContent=carry?'移动速度影响往返用时；装载上限由搬运等级决定。':
      farm?'这里显示游戏等级表中的参数。每次放牧的产物、数量和概率取决于物种和有效等级。':
      r.work<=-100?'词条合计达到 −100% 或更低，公式按零显示；游戏运行时的最低工作速度尚未核实。':
      '标准工况只计工作等级和自身词条。设施、研究、伙伴技能、帕鲁之魂、饮食及游戏设置会影响实际产速。';
    const unknown=selected.filter(key=>!byTrait.has(key));
    if(unknown.length) el('scope').textContent+=' 仓库中的 '+unknown.join('、')+' 暂无效果数据，结果仅计已知加成。';
    el('ranch-link').hidden=!farm||!r.rank;
    el('ranch-link').href='#work/ranch'+(known&&natural?'/'+encodeURIComponent(palKey):'');
    el('scale').innerHTML=data.levels[job].map((value,i)=>'<span aria-current="'+(i+1===r.rank)+'"><small>Lv. '+(i+1)+'</small><b>'+number(value)+'</b></span>').join('');
  }
  function render(){updateTraits();updateResult();}
  function pick(id,requestedJob='') {
    sourceId=id;currentRecord=null;palKey='';selected=[];note('');
    const row=sourceRows.find(r=>r.id===id);
    if(row) {
      palKey=row.palKey;currentRecord=row.record||null;
      selected=[...new Set(currentRecord?.traits||[])].slice(0,4);
      const work=workOf(palKey);
      if(requestedJob)job=requestedJob;
      else if(work.length&&!work.some(pair=>pair[0]===job))job=work[0][0];
      level=work.find(pair=>pair[0]===job)?.[1]||(defaults.has(palKey)?0:1);
    } else level=origin==='manual'?1:0;
    render();
  }
  function readSource() {
    if(origin==='manual') return [];
    if(origin==='dex') return species.map(p=>({id:p.key,palKey:p.key,label:p.zh+' #'+p.dex,terms:[p.zh,p.en,p.key,p.dex,...(p.aliases||[])]}));
    return inventory.list().map(record=>{
      const p=defaults.get(record.speciesKey),name=record.name||p?.zh||record.speciesKey;
      const label=name+(p&&p.zh!==name?' · '+p.zh:'')+' · '+({M:'雄',F:'雌'}[record.sex]||'性别未知')+' · '+record.id.slice(-8);
      return {id:record.id,palKey:record.speciesKey,label,record,terms:[label,p?.en,p?.dex,record.speciesKey,...(p?.aliases||[])]};
    });
  }
  function listSource() {
    const query=norm(el('pal-query').value).replace(/^#/,'');
    const rows=sourceRows.filter(r=>r.terms.some(s=>norm(s).includes(query)));
    const select=el('pal');select.replaceChildren(new Option(rows.length?'请选择帕鲁':origin==='warehouse'&&!sourceRows.length?'当前世界的仓库为空':'没有匹配的帕鲁',''));
    for(const row of rows) select.add(new Option(row.label,row.id));
    // Searching for another Pal does not discard the current calculation.
    if(sourceId&&!rows.some(r=>r.id===sourceId)) {
      const current=sourceRows.find(r=>r.id===sourceId);
      if(current) select.add(new Option('当前 · '+current.label,current.id));
    }
    select.value=sourceId;
    return rows;
  }
  function setOrigin(value,initial='',requestedJob='') {
    origin=value;el('origin').value=value;el('picker').hidden=value==='manual';el('pal-query').value='';
    el('pal-label').textContent=value==='warehouse'?'仓库个体':'图鉴物种';
    sourceRows=readSource();sourceId=initial;listSource();pick(initial,requestedJob);
  }
  el('origin').addEventListener('change',()=>setOrigin(el('origin').value));
  el('pal').addEventListener('change',()=>pick(el('pal').value));
  el('pal-query').addEventListener('input',listSource);
  el('pal-query').addEventListener('keydown',e=>{
    if(e.key!=='Enter'||e.isComposing||e.keyCode===229) return;
    const rows=listSource(),q=norm(el('pal-query').value).replace(/^#/,'');
    const exact=rows.filter(row=>row.terms.some(s=>s&&norm(s)===q));
    const row=exact.length===1?exact[0]:rows.length===1?rows[0]:null;
    if(row) {e.preventDefault();el('pal').value=row.id;pick(row.id);}
  });
  el('job').addEventListener('change',()=>{
    job=el('job').value;
    if(origin!=='manual'&&defaults.has(palKey)) level=workOf(palKey).find(pair=>pair[0]===job)?.[1]||0;
    note('');render();
  });
  el('level').addEventListener('change',()=>{level=Number(el('level').value);updateResult();});
  function toggle(key,from) {
    if(selected.includes(key)) selected=selected.filter(existing=>existing!==key);
    else if(selected.length<4) selected.push(key);
    else {note('每只帕鲁最多有四个被动词条。');return;}
    note('');render();
    const next=[...el(from).querySelectorAll('button')].find(b=>(b.dataset.weTrait||b.dataset.weRemove)===key);
    (next||el('query')).focus({preventScroll:true});
  }
  el('options').addEventListener('click',e=>{const button=e.target.closest('button[data-we-trait]');if(button)toggle(button.dataset.weTrait,'options');});
  el('picked').addEventListener('click',e=>{const button=e.target.closest('button[data-we-remove]');if(button)toggle(button.dataset.weRemove,'picked');});
  for(const [id,all] of [['relevant',false],['all',true]]) el(id).addEventListener('click',()=>{showAll=all;el('query').value='';note('');updateTraits();});
  el('query').addEventListener('input',()=>{note('');updateTraits();});
  el('query').addEventListener('keydown',e=>{
    if(e.key!=='Enter'||e.isComposing||e.keyCode===229) return;
    const names=el('query').value.split(/[、，,;；\n]/).map(norm).filter(Boolean);
    if(!names.length) return;
    e.preventDefault();const next=new Set(selected);
    for(const name of names) {
      const exact=traits.find(t=>[t.zh,t.en,t.key].some(s=>norm(s)===name));
      const matches=exact?[exact]:traits.filter(t=>[t.zh,t.en,t.key].some(s=>norm(s).includes(name)));
      if(matches.length!==1){note(matches.length?'有多个匹配，请点选具体词条。':'没有找到“'+name+'”。');return;}
      next.add(matches[0].key);
    }
    if(next.size>4){note('每只帕鲁最多有四个被动词条。');return;}
    selected=[...next];el('query').value='';note('');render();
  });
  function refreshStock() {
    if(origin!=='warehouse') return;
    sourceRows=readSource();const fresh=sourceRows.find(row=>row.id===sourceId);
    if(sourceId&&!fresh){sourceId='';listSource();pick('');return;}
    if(fresh&&(fresh.palKey!==palKey||JSON.stringify(fresh.record.traits)!==JSON.stringify(currentRecord?.traits))) {
      listSource();pick(fresh.id);return;
    }
    listSource();
  }
  inventory.subscribe(()=>{if(!root.hidden) refreshStock();});
  return {show(argument,requestedJob) {
    if(defaults.has(argument)) {
      setOrigin('dex',argument,jobs.includes(requestedJob)?requestedJob:'');return;
    }
    if(jobs.includes(argument)) {setOrigin('manual');job=argument;level=1;}
    refreshStock();render();
  }};
}
if(typeof module!=='undefined'&&module.exports) module.exports={calculateWorkEfficiency,initWorkEfficiency};

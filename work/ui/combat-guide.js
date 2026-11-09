function initCombatGuide({data,escape}) {
  const el=id=>document.getElementById('cg-'+id),types=data.elements,keys=Object.keys(types);
  const labels={'1':'克制','-1':'被抵抗','0':'通常',unknown:'待核实'},symbols={'1':'↑','-1':'↓','0':'·',unknown:'？'};
  const names=new Map(data.status.map(s=>[s.id,s])),groups=[...new Set(data.status.map(s=>s.group))];
  let group='',current=null;
  function match(a,d) {
    if(a===d && a!=='neutral' || a==='neutral'&&d==='dark')return 'unknown';
    if(data.strong[a].includes(d))return 1;
    if(a!=='neutral'&&data.strong[d].includes(a))return -1;
    return 0;
  }
  for(const id of ['attack','defend','second']) for(const key of keys) el(id).add(new Option(types[key],key));
  // The hit areas follow the centres of the nine emblems on the 1200 × 675 guide plate.
  const emblems={
    water:[30.87,21.55],electric:[15.05,42.96],fire:[46.90,42.96],
    ice:[65.49,42.96],dragon:[83.79,42.96],ground:[21.40,75.11],
    grass:[41.07,75.11],neutral:[65.49,75.11],dark:[83.79,75.11]
  };
  const weaknesses=Object.fromEntries(keys.map(key=>[key,keys.filter(other=>data.strong[other].includes(key))]));
  function typeNames(list) {return list.map(k=>types[k]).join('、')||'没有';}
  for(const [key,[x,y]] of Object.entries(emblems)) {
    const button=document.createElement('button'),label=document.createElement('span');
    button.type='button';button.dataset.cgPick=key;
    button.style.left=x+'%';button.style.top=y+'%';
    if(y>60)button.className='cg-label-below';
    button.setAttribute('aria-label','选择'+types[key]+'属性攻击');
    button.title=types[key]+'属性　优势：'+typeNames(data.strong[key])+'　弱点：'+typeNames(weaknesses[key]);
    label.textContent=types[key];button.append(label);el('chains').append(button);
  }
  el('chains').addEventListener('click',e=>{
    const key=e.target.closest('[data-cg-pick]')?.dataset.cgPick;if(!key)return;
    el('attack').value=key;el('defend').value=data.strong[key][0]||'neutral';el('second').value='';update();
  });
  el('attack').value='fire';el('defend').value='grass';
  el('matrix').insertAdjacentHTML('beforeend','<thead><tr><th scope="col">攻／守</th>'+
    keys.map(k=>'<th scope="col">'+types[k]+'</th>').join('')+'</tr></thead><tbody>'+keys.map(a=>'<tr><th scope="row">'+types[a]+'</th>'+keys.map(d=>{
      const mark=match(a,d),desc=types[a]+'攻'+types[d]+' · '+labels[mark];
      return '<td><button type="button" data-cg-a="'+a+'" data-cg-d="'+d+'" data-cg-result="'+mark+'" title="'+desc+'" aria-label="'+desc+'">'+symbols[mark]+'</button></td>';
    }).join('')+'</tr>').join('')+'</tbody>');
  function update() {
    const a=el('attack').value,d=el('defend').value;
    if(el('second').value===d)el('second').value='';
    for(const option of el('second').options)option.disabled=option.value===d;
    const target=[d,el('second').value].filter(Boolean),results=target.map(key=>match(a,key));
    const score=results.includes('unknown')?null:results.reduce((sum,n)=>sum+n,0);
    const outcome=score===null?'有待核实的属性组合':score>1?'双重克制':score===1?'克制':score===0?(results.includes(1)?'克制与抵抗相抵':'通常'):(score<-1?'双重抵抗':'被抵抗');
    el('current').innerHTML='<strong>'+types[a]+' → '+target.map(t=>types[t]).join('／')+'　'+outcome+'</strong><span>'+target.map((t,i)=>types[a]+'攻'+types[t]+'：'+labels[results[i]]).join('　·　')+'</span>';
    el('affinities').innerHTML='<dt>'+types[a]+'系克制</dt><dd>'+typeNames(data.strong[a])+'</dd><dt>'+types[a]+'系弱点</dt><dd>'+typeNames(weaknesses[a])+'</dd>';
    for(const b of el('chains').querySelectorAll('button'))b.setAttribute('aria-pressed',String(b.dataset.cgPick===a));
    for(const b of el('matrix').querySelectorAll('button'))b.setAttribute('aria-pressed',String(b.dataset.cgA===a&&target.includes(b.dataset.cgD)));
  }
  for(const id of ['attack','defend','second']) el(id).addEventListener('change',update);
  el('matrix').addEventListener('click',e=>{
    const b=e.target.closest('[data-cg-a]');if(!b)return;
    el('attack').value=b.dataset.cgA;el('defend').value=b.dataset.cgD;el('second').value='';update();
  });
  el('groups').innerHTML=['',...groups].map(g=>'<button type="button" data-cg-group="'+escape(g)+'">'+escape(g||'全部状态')+'</button>').join('');
  function renderState(id) {
    const shown=data.status.filter(s=>!group||s.group===group);
    current=shown.find(s=>s.id===id)||shown[0];
    for(const b of el('groups').children)b.setAttribute('aria-pressed',String(b.dataset.cgGroup===group));
    el('states').innerHTML=shown.map(s=>'<a href="#guide/status/'+encodeURIComponent(s.id)+'"'+(s===current?' aria-current="true"':'')+'><small>'+escape(s.group)+'</small><strong>'+escape(s.name)+'</strong></a>').join('');
    const s=current;
    const examples=s.moves?.length?'<details class="cg-moves"><summary>可施加该状态的主动技能（'+s.moves.length+'）</summary><p>数值是技能记录的积蓄值。</p><ul>'+s.moves.map(([name,type,value,moveId])=>'<li><a href="#details/active/move/'+encodeURIComponent(moveId)+'">'+escape(name)+'</a><small>'+escape(types[type]||type)+' · '+escape(value)+'</small></li>').join('')+'</ul></details>':'';
    el('state').innerHTML='<small>'+escape(s.group)+'</small><h2>'+escape(s.name)+'</h2><p>'+escape(s.text)+'</p>'+
      (s.medicine?'<p>对应药品　<a href="#items/'+encodeURIComponent(s.medicine[0])+'">'+escape(s.medicine[1])+'</a>（等级 '+s.rank+'）</p>':'')+
      (s.group==='技能异常'||s.group==='野外状态'?'<a href="#details/capture">前往捕捉概率计算 →</a>':'')+examples;
  }
  el('groups').addEventListener('click',e=>{
    const btn=e.target.closest('[data-cg-group]');if(!btn)return;
    group=btn.dataset.cgGroup;renderState(current?.id);
    history.replaceState(history.state,'','#guide/status/'+encodeURIComponent(current.id));
  });
  return {show(tab,id) {
    const statuses=tab==='status';
    el('elements').hidden=statuses;el('status').hidden=!statuses;
    el('title').textContent=statuses?'异常状态':'属性克制图';
    el('description').textContent=statuses?'查看技能异常、野外状态与据点疾病的效果及应对方式。':'点击图中的属性，查看它的优势与弱点。';
    if(statuses){
      if(!id || (group&&names.get(id)?.group!==group))group='';
      renderState(id);
    }else {
      if(!el('diagram').hasAttribute('src'))el('diagram').src=data.chart;
      update();
    }
  }};
}

function initPartnerSkills({data,species,avatar,listAvatars,fillPortraits,onPalChange,escape,normalize}) {
  const el=id=>document.getElementById('pk-'+id);
  const types={combat:'战斗',work:'工作',travel:'骑乘与移动',explore:'探索与负重',loot:'掉落与垂钓',catch:'捕捉',care:'支援与恢复'};
  const elements={neutral:'无',fire:'火',water:'水',grass:'草',electric:'雷',ice:'冰',ground:'地',dark:'暗',dragon:'龙'};
  const order=species.filter(p=>Object.hasOwn(data,p.key));
  const byKey=new Map(order.map(p=>[p.key,p]));
  const portraits=new Map(species.map((p,i)=>[p.key,listAvatars?.[i]]));
  const index=new Map(order.map(p=>[p.key,normalize([p.zh,p.en,p.key,p.dex,...(p.aliases||[]),data[p.key].name,data[p.key].effect].join(' '))]));
  const checked=new Set();
  let selected=null,rank=0,shown=[],rowNodes=new Map();
  for(const [value,label] of Object.entries(elements)) el('element').add(new Option(label+'属性',value));
  el('types').innerHTML=Object.entries({all:'全部',...types}).map(([key,name])=>'<button type="button" data-pk-type="'+key+'" aria-pressed="false">'+name+'</button>').join('');
  function clear() {el('query').value='';el('element').value='';checked.clear();}
  function effectWithLinks(text,mentions=[]) {
    if(!mentions.length)return escape(text);
    const links=new Map(mentions);
    const pattern=mentions.map(([name])=>name.replace(/[.*+?^$(){}|[\]\\]/g,'\\$&')).join('|');
    return text.split(new RegExp('('+pattern+')','g')).map(part=>links.has(part)
      ?'<a href="'+escape(links.get(part))+'">'+escape(part)+'</a>':escape(part)).join('');
  }
  function detail() {
    if(!selected){el('detail').hidden=true;return;}
    const pal=byKey.get(selected),record=data[selected];
    const encoded=encodeURIComponent(selected);
    let text=record.effect;
    if(rank) for(const [token,curve] of Object.entries(record.curves||{})) text=text.split(token).join(String(curve[rank-1]));
    const ranks=!!record.curves||!!record.stats;
    const equipment=(record.gear||[]).map(([id,name])=>'<a href="#items/'+encodeURIComponent(id)+'">'+escape(name)+'</a>').join('、');
    const params=(record.stats||[]).map(([label,values])=>'<div><dt>'+escape(label)+'</dt><dd>'+escape(rank?String(values[rank-1]):values.join(' / '))+'</dd></div>').join('');
    el('detail').hidden=false;
    el('detail').innerHTML='<div class="pk-hero">'+avatar(pal)+'<div><small>#'+escape(pal.dex)+' · '+escape(pal.zh)+'</small><h2>'+escape(record.name)+'</h2></div></div>'+
      '<div class="pk-badges">'+record.tags.map(t=>'<span>'+types[t]+'</span>').join('')+'</div>'+
      (ranks?'<label class="pk-level">技能等级<select id="pk-level"><option value="0">全部等级</option>'+[1,2,3,4,5].map(n=>'<option value="'+n+'"'+(rank===n?' selected':'')+'>Lv. '+n+'</option>').join('')+'</select></label>':'')+
      '<p class="pk-effect">'+effectWithLinks(text,record.mentions)+'</p>'+(rank&&/\([-\d.]+~[-\d.]+\)/.test(text)?'<small>保留区间的项目仅有等级范围。</small>':'')+
      (params?'<dl class="pk-numbers">'+params+'</dl>':'')+
      (equipment||record.tech?'<p class="pk-gear">解锁装备　'+(equipment||'伙伴装备')+(record.tech?' · 科技 Lv. '+record.tech:'')+'</p>':'')+
      '<div class="pk-links"><a href="#paldex/'+encoded+'">查看帕鲁图鉴 →</a>'+(record.tags.includes('work')?'<a href="#work/suitability/'+encoded+'">查看工作适应性 →</a>':'')+(record.ranch?'<a href="#work/ranch/'+encoded+'">查看牧场产物 →</a>':'')+'</div>';
  }
  function draw(requested,updateAddress=false) {
    const query=normalize(el('query').value).replace(/^#/,''),element=el('element').value,number=/^(\d{1,3})([a-z]?)$/.exec(query);
    const matches=p=>number?Number.parseInt(p.dex,10)===Number(number[1])&&(!number[2]||p.dex.toLowerCase().endsWith(number[2])):!query||index.get(p.key).includes(query);
    shown=order.filter(p=>(!element||p.types.includes(element))&&matches(p)&&
      (!checked.size||data[p.key].tags.some(t=>checked.has(t))));
    const keep=shown.some(p=>p.key===requested);
    const next=keep?requested:shown[0]?.key||null;
    if(selected!==next)rank=0;
    selected=next;
    if(!updateAddress&&selected&&history.state?.partnerRank?.key===selected)rank=history.state.partnerRank.rank;
    for(const button of el('types').children) {
      const t=button.dataset.pkType;button.setAttribute('aria-pressed',String(t==='all'?!checked.size:checked.has(t)));
    }
    el('count').textContent=shown.length+' / '+order.length+' 种帕鲁';
    const nodes=shown.map(p=>{
      let a=rowNodes.get(p.key);
      if(!a) {
        a=document.createElement('a');a.href='#details/partner/'+encodeURIComponent(p.key);a.dataset.pkPal=p.key;
        a.innerHTML=(portraits.get(p.key)||avatar(p))+'<span class="pk-row-copy"><span class="pk-pal-name"><b>'+escape(p.zh)+'</b> <small>#'+escape(p.dex)+'</small></span><span class="pk-skill-name">'+escape(data[p.key].name)+'</span></span>';
        rowNodes.set(p.key,a);
      }
      if(p.key===selected)a.setAttribute('aria-current','true');else a.removeAttribute('aria-current');
      return a;
    });
    el('list').replaceChildren(...nodes);
    fillPortraits?.(el('list'));
    if(!shown.length)el('list').textContent='没有匹配的帕鲁。';
    const address=updateAddress && /^#details\/partner\//.test(location.hash)
      ? (selected?'#details/partner/'+encodeURIComponent(selected):'#details/partner') : location.href;
    history.replaceState({...history.state,partnerFilters:{query:el('query').value,element,types:[...checked]}},'',address);
    onPalChange?.(selected);
    detail();
  }
  el('types').addEventListener('click',e=>{
    const t=e.target.closest('[data-pk-type]')?.dataset.pkType;if(!t)return;
    if(t==='all')checked.clear();else if(checked.has(t))checked.delete(t);else checked.add(t);
    el('list').scrollTop=0;draw(selected,true);
  });
  el('query').addEventListener('input',()=>{el('list').scrollTop=0;draw(selected,true);});
  el('element').addEventListener('change',()=>{el('list').scrollTop=0;draw(selected,true);});
  el('reset').addEventListener('click',()=>{clear();el('list').scrollTop=0;draw(null,true);});
  el('detail').addEventListener('change',e=>{
    if(e.target.id!=='pk-level')return;
    rank=Number(e.target.value);
    history.replaceState({...history.state,partnerRank:{key:selected,rank}},'',location.href);
    detail();el('level').focus();
  });
  return {show(key) {
    const saved=history.state?.partnerFilters;
    if(saved) {
      clear();el('query').value=saved.query;el('element').value=saved.element;
      for(const type of saved.types)checked.add(type);
    }
    if(!key||key==='work') {if(!saved)clear();if(key==='work'&&!saved)checked.add('work');el('list').scrollTop=0;draw(null);return;}
    if(!byKey.has(key)) {clear();draw(null);el('detail').hidden=false;el('detail').textContent='该形态暂无独立的伙伴技能记录。';return;}
    if(!saved&&!shown.some(p=>p.key===key))clear();
    draw(key);
    const link=rowNodes.get(key);if(link)el('list').scrollTop+=link.getBoundingClientRect().top-el('list').getBoundingClientRect().top-el('list').clientHeight/3;
  }};
}

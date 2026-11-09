function relicProductOdds(tier,pools) {
  const products=new Map();
  for(const slot of tier.slots) {
    const pool=pools[slot.pool], total=pool.reduce((sum,row)=>sum+row[3],0);
    for(const [id,min,max,weight] of pool) {
      const p=slot.probability/100*weight/total;
      let product=products.get(id);
      if(!product) {
        product={id,probability:0,min,max:0,slots:0};products.set(id,product);
      }
      product.probability+=(1-product.probability)*p;
      product.min=Math.min(product.min,min);product.max+=max;product.slots++;
    }
  }
  return products;
}

function simulateRelic(tier,pools,random=Math.random) {
  const result=new Map();
  for(const slot of tier.slots) {
    if(random()*100>=slot.probability) continue;
    const entries=pools[slot.pool];
    let draw=random()*entries.reduce((sum,row)=>sum+row[3],0);
    for(const [id,min,max,weight] of entries) {
      draw-=weight;
      if(draw>=0) continue;
      const quantity=min+Math.floor(random()*(max-min+1));
      result.set(id,(result.get(id)||0)+quantity);
      break;
    }
  }
  return result;
}

function initRelicGuide({root,data,itemIcon,items}) {
  const el=id=>root.querySelector('#rl-'+id);
  const byId=new Map(items.map(i=>[i.id,i]));
  const byTier=new Map(data.tiers.map(t=>[t.id,t]));
  const odds=new Map(data.tiers.map(t=>[t.id,relicProductOdds(t,data.pools)]));
  const rank=['普通','少见','稀有','史诗','传说'];
  const escape=s=>String(s).replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'})[c]);
  const name=id=>escape(byId.get(id)?.name||id);
  const image=id=>{const src=itemIcon(id);return src?'<img src="'+src+'" alt="" loading="lazy">':'';};
  const href=id=>'#items/'+encodeURIComponent(id);
  const inputHref=id=>'#work/relic/'+encodeURIComponent(id);
  const targetHref=id=>'#work/relic/item/'+encodeURIComponent(id);
  const pct=probability=>new Intl.NumberFormat('zh-CN',{maximumFractionDigits:probability<.0001?5:3}).format(probability*100)+'%';
  const range=(a,b)=>a===b?String(a):a+'–'+b;
  const terms=new Map(data.outputs.map(id=>{
    const v=byId.get(id);
    return [id,[v.name,v.en,id].join(' ').toLowerCase()];
  }));
  const filterMatches=(id,term)=>terms.get(id).includes(term.trim().toLowerCase());
  const categories=[...new Set(data.outputs.map(id=>byId.get(id).category))].sort((a,b)=>a.localeCompare(b,'zh-CN'));
  el('category').add(new Option('全部类别',''));
  for(const type of categories) el('category').add(new Option(type,type));
  const materials=[['WorldTreeIngot',50],['Wood_WorldTree',50],['PalCrystal_Ex',30],['AncientParts2',20]];
  el('materials').innerHTML=materials.map(([id,n])=>'<a href="'+href(id)+'">'+name(id)+' ×'+n+'</a>').join('、');
  const slotNames=['神秘木材','帕鲁树晶矿','辉石①','辉石②','辉石③','辉石④','辉石⑤','耗材植入体','古代文明核心','果实及养成物品①','果实及养成物品②','果实及养成物品③','装备设计图','稀有耗材植入体'];
  let tier=data.tiers[0], target=data.outputs[0], simulation=null;
  function titleItem(id) {
    return '<div class="rl-identity">'+image(id)+'<div><strong>'+name(id)+'</strong><small>'+escape(byId.get(id).category)+'</small></div></div>';
  }
  function renderProducts() {
    const values=[...odds.get(tier.id).values()].filter(row=>filterMatches(row.id,el('filter').value)&&(!el('category').value||byId.get(row.id).category===el('category').value));
    values.sort((a,b)=>b.probability-a.probability||byId.get(a.id).name.localeCompare(byId.get(b.id).name,'zh-CN'));
    el('count').textContent=values.length+' 种';
    el('results').innerHTML=values.map(v=>'<a class="rl-product" href="'+targetHref(v.id)+'" title="比较五档遗物的产出概率">'+image(v.id)+'<span><b>'+name(v.id)+'</b><small>获得 '+range(v.min,v.max)+' 个</small></span><strong>'+pct(v.probability)+'</strong></a>').join('')||'<p class="rl-empty">没有符合条件的产物。</p>';
  }
  function renderSim() {
    el('sim').innerHTML=simulation?[...simulation].map(([id,n])=>'<a class="rl-chip" href="'+href(id)+'">'+image(id)+name(id)+' ×'+n+'</a>').join(''):'点击按钮查看一次随机结果。';
  }
  function renderInput() {
    el('tiers').innerHTML=data.tiers.map((t,i)=>'<a href="'+inputHref(t.id)+'" class="rl-tier" data-grade="'+i+'" '+(t===tier?'aria-current="true"':'')+'>'+image(t.id)+'<span><small>'+rank[i]+'</small><b>'+name(t.id).replace('古代文明遗物','遗物')+'</b></span></a>').join('');
    el('selected').innerHTML=titleItem(tier.id)+'<div class="rl-values"><span>消耗遗物</span><b>1 件</b><span>处理工作量</span><b>'+tier.work.toLocaleString('zh-CN')+'</b></div><a class="rl-item-link" href="'+href(tier.id)+'">查看遗物获取方式 ↗</a>';
    el('fixed').innerHTML=[...odds.get(tier.id).values()].filter(v=>v.probability===1).map(v=>'<a class="rl-chip" href="'+href(v.id)+'">'+image(v.id)+name(v.id)+' ×'+range(v.min,v.max)+'</a>').join('');
    el('slots').innerHTML=tier.slots.map((s,i)=>'<div><span>'+slotNames[i]+'</span><b>'+pct(s.probability/100)+'</b></div>').join('');
    renderProducts();renderSim();
  }
  function renderPickList() {
    const list=data.outputs.filter(id=>filterMatches(id,el('item-filter').value));
    el('item-count').textContent=list.length+' 种';
    el('item-list').innerHTML=list.map(id=>'<a class="rl-product" href="'+targetHref(id)+'" '+(id===target?'aria-current="true"':'')+'>'+image(id)+'<span><b>'+name(id)+'</b><small>'+escape(byId.get(id).category)+'</small></span></a>').join('')||'<p class="rl-empty">没有匹配的产物。</p>';
  }
  function renderReverse() {
    el('target').innerHTML=titleItem(target)+'<a class="rl-item-link" href="'+href(target)+'">查看物品图鉴 ↗</a>';
    el('odds').innerHTML='<h3>每件遗物的产出概率</h3><div class="rl-odds-head"><span>投入遗物</span><span>获得数量</span><span>至少一次</span></div>'+data.tiers.map((t,i)=>{
      const o=odds.get(t.id).get(target);
      return '<a class="rl-odds-row" href="'+inputHref(t.id)+'"><span>'+image(t.id)+name(t.id).replace('古代文明遗物','遗物')+'</span><span>'+(o?range(o.min,o.max):'—')+'</span><b>'+(o?pct(o.probability):'无此产物')+'</b></a>';
    }).join('');
    renderPickList();
  }
  el('roll').addEventListener('click',()=>{simulation=simulateRelic(tier,data.pools);renderSim();});
  el('filter').addEventListener('input',renderProducts);
  el('category').addEventListener('change',renderProducts);
  el('item-filter').addEventListener('input',renderPickList);
  return {show(id,mode) {
    const reverse=mode==='item';
    if(reverse) target=data.outputs.includes(id)?id:target;
    else {
      const next=byTier.get(id)||tier;
      if(next!==tier) simulation=null;
      tier=next;
    }
    for(const a of root.querySelectorAll('[data-rl-mode]')) {
      const isItem=a.dataset.rlMode==='item';
      a.href=isItem?targetHref(target):inputHref(tier.id);
      if(isItem===reverse) a.setAttribute('aria-current','page');else a.removeAttribute('aria-current');
    }
    el('input').hidden=reverse;el('reverse').hidden=!reverse;
    if(reverse) renderReverse();else renderInput();
  }};
}

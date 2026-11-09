function initFacilities({data,items,itemIcon,escape,normalize}) {
  const el=id=>document.getElementById('fc-'+id);
  const facilities=data.facilities,byId=new Map(facilities.map(f=>[f.id,f]));
  const catalog=new Map(items.map(item=>[item.id,item])),products=new Map();
  for(const item of items) {
    const stations=new Set((item.recipes||[]).flatMap(recipe=>recipe.stations||[]));
    for(const station of stations) {
      if(!products.has(station)) products.set(station,[]);
      products.get(station).push(item);
    }
  }
  const jobs=f=>[...new Set([...f.required.map(([job])=>job),...f.jobs])];
  const jobNames=['生火','浇水','播种','发电','手工作业','采集','伐木','采矿','制药','冷却','搬运','牧场'];
  for(const name of new Set(facilities.map(f=>f.category))) el('category').add(new Option(name,name));
  for(const job of jobNames) el('job').add(new Option(job,job));
  const related=f=>[...jobs(f),...(f.id==='Beta_Wave_Generator'?jobNames:[])];
  const choices=facilities.map(f=>({f,search:normalize([f.name,f.id,f.code||'',f.group,f.category,f.purpose,...related(f)].join(' '))}));
  const href=id=>'#work/facilities/'+encodeURIComponent(id);
  let selected,shown=[];
  function image(src){return src?'<img alt="" loading="lazy" decoding="async" src="'+escape(src)+'">':'';}
  function itemLink(id,label,qty) {
    const icon=image(itemIcon(id));
    return '<a class="fc-item" href="#items/'+encodeURIComponent(id)+'">'+icon+escape(label)+(qty?' <b>×'+qty+'</b>':'')+'</a>';
  }
  function detail(f) {
    const panel=el('detail');
    if(!f){panel.hidden=true;panel.replaceChildren();return;}
    panel.hidden=false;
    const source='https://paldb.cc/cn/'+encodeURIComponent(f.id);
    const using=jobs(f),isBonus=f.group==='效率强化',links=[];
    for(const [id] of f.production||[]) links.push(['#work/production/'+encodeURIComponent(id),'查看据点生产条件与产物']);
    if(f.id==='Ranch') links.push(['#work/ranch','查看牧场产物']);
    if(f.id==='Pal_Expedition_Station') links.push(['#work/expeditions','查看远征要求与奖励']);
    if(f.id==='Ancient_Relic_Recycler') links.push(['#work/relic','查询遗物转换']);
    if(f.id==='Pal_Surgery_Table') links.push(['#details/surgery','查看手术项目']);
    if(f.id==='Skillfruit_Orchard') links.push(['#details/fruits','查看技能果实']);
    const stats=[];
    if(f.level)stats.push(['解锁科技','Lv. '+f.level]);
    if(f.energy==='Electric')stats.push(['能源','需要供电']);
    if(f.buildWork>0)stats.push(['建造工作量（原始值）',f.buildWork.toLocaleString('zh-CN')]);
    const work=f.required.map(([job,lv])=>job==='任意'?'任意种类帕鲁':job+' Lv. '+lv);
    const tags=work.concat(f.jobs.filter(j=>!f.required.some(([job])=>job===j)));
    const productList=products.get(f.name)||[];
    panel.innerHTML='<header>'+image(f.icon)+'<div><span>'+escape(f.category)+' / '+escape(f.group)+'</span><h2>'+escape(f.name)+'</h2></div></header>'+
      (f.purpose?'<p class="fc-purpose">'+escape(f.purpose)+'</p>':'')+
      (stats.length?'<dl>'+stats.map(([name,value])=>'<div><dt>'+name+'</dt><dd>'+escape(value)+'</dd></div>').join('')+'</dl>':'')+
      (tags.length?'<h3>'+(isBonus?'加成工作':'工作适应性')+'</h3><p>'+tags.map(escape).join(' · ')+'</p>':'')+
      (f.effects.length?'<h3>设施效果</h3><p>'+f.effects.map(escape).join(' · ')+'</p>':'')+
      (isBonus?'<p class="fc-note">同种设施的据点效果不叠加。</p>':'')+
      (f.materials.length?'<h3>建造材料</h3><div class="fc-items">'+f.materials.map(([id,count,name])=>itemLink(id,catalog.get(id)?.name||name,count)).join('')+'</div>':'')+
      (productList.length?'<details id="fc-products"><summary>可制作物品 · '+productList.length+' 种</summary><div class="fc-product-body"></div></details>':'')+
      (using.some(j=>jobNames.includes(j))?'<h3>计算帕鲁工作效率</h3><div class="fc-actions">'+using.filter(j=>jobNames.includes(j)).map(j=>'<a href="#work/efficiency/'+encodeURIComponent(j)+'">'+escape(j)+'</a>').join('')+'</div>':'')+
      (links.length?'<div class="fc-actions">'+links.map(([url,name])=>'<a href="'+url+'">'+name+'</a>').join('')+'</div>':'')+
      '<footer><a href="'+source+'" target="_blank" rel="noopener noreferrer">PalDB · 设施资料 ↗</a> · <a href="https://github.com/Dhampyru/Palworld-Extracted/blob/d38105b4dc0cf6f36c5035997fbe54205bb2d753/Pal/Content/Pal/DataTable/MapObject/Building/DT_BuildObjectDataTable_Common.json" target="_blank" rel="noopener noreferrer">游戏建造表 ↗</a></footer>';
    const fold=el('products');
    if(fold)fold.addEventListener('toggle',()=>{
      if(!fold.open||fold.dataset.ready)return;
      fold.dataset.ready='true';
      fold.querySelector('.fc-product-body').innerHTML='<label>查找可制作物品<input type="search" placeholder="物品名称或编号" spellcheck="false"></label><div class="fc-items"></div><button class="fc-more" type="button" hidden>显示更多</button>';
      const input=fold.querySelector('input'),list=fold.querySelector('.fc-items'),more=fold.querySelector('.fc-more');
      let limit=48;
      const paint=()=>{
        const query=normalize(input.value);
        const matches=productList.filter(item=>!query||[item.name,item.en,item.id].some(x=>normalize(x).includes(query)));
        list.innerHTML=matches.length?matches.slice(0,limit).map(item=>itemLink(item.id,item.name)).join(''):'没有匹配的物品。';
        more.hidden=matches.length<=limit;
        more.textContent='显示更多（'+Math.min(limit,matches.length)+' / '+matches.length+'）';
      };
      more.addEventListener('click',()=>{limit+=48;paint();});
      input.addEventListener('input',()=>{limit=48;paint();});paint();
    });
  }
  function refresh(id) {
    const q=normalize(el('query').value),cat=el('category').value,job=el('job').value;
    shown=choices.filter(({f,search})=>(!cat||f.category===cat)&&(!job||related(f).includes(job))&&(!q||search.includes(q))).map(({f})=>f);
    selected=shown.find(f=>f.id===id)||shown[0];
    el('count').textContent=shown.length+' 项';
    el('list').innerHTML=shown.length?shown.map(f=>'<a class="fc-choice" href="'+href(f.id)+'"'+(f===selected?' aria-current="true"':'')+'>'+image(f.icon)+'<span><strong>'+escape(f.name)+'</strong><small>'+escape(f.group)+(f.level?' · Lv. '+f.level:'')+'</small></span></a>').join(''):'<p>没有匹配的设施。</p>';
    detail(selected);
  }
  for(const key of ['query','category','job'])el(key).addEventListener(key==='query'?'input':'change',()=>{
    refresh(selected?.id);
    history.replaceState(history.state,'',selected?href(selected.id):'#work/facilities');
  });
  return {show(id) {
    if(id&&byId.has(id)&&!shown.some(f=>f.id===id)){el('query').value='';el('category').value='';el('job').value='';}
    refresh(id||selected?.id);
  }};
}

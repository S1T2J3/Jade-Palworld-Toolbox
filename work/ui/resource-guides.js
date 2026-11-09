function initResourceGuides({data,itemIcon}){
  const esc=value=>String(value??'').replace(/[&<>"']/g,ch=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[ch]));
  const normalize=value=>String(value).normalize('NFKC').trim().toLowerCase();
  const num=n=>n.toLocaleString('zh-CN',{maximumFractionDigits:1});
  const href=(path,id)=>path+'/'+encodeURIComponent(id);
  const base={gathering:'#guide/gathering',production:'#work/production'};
  const link=(url,label)=>'<a href="'+esc(url)+'">'+esc(label)+'</a>';
  const item=(id,name)=>link(href('#items',id),name);
  const icon=id=>{const src=itemIcon(id);return src?'<img class="rg-icon" loading="lazy" decoding="async" alt="" src="'+esc(src)+'">':'';};
  const links=markup=>'<div class="rg-links">'+markup.join('')+'</div>';
  const section=(title,inner)=>inner?'<section class="rg-section"><h3>'+title+'</h3>'+inner+'</section>':'';
  const stats=entries=>'<dl class="rg-stats">'+entries.map(([k,v])=>'<div><dt>'+esc(k)+'</dt><dd>'+esc(v)+'</dd></div>').join('')+'</dl>';
  const gatherById=new Map(data.gathering.map(r=>[r.id,r]));
  function productionGroup(r){
    if(!r.facility)return '仅有生产参数';
    return r.auto?'自动提炼':r.jobs.map(pair=>pair[0]).join('、')||'设施作业';
  }
  function citation(urls){
    return '<footer class="rg-sources">资料来源'+[...new Set(urls)].map(u=>{
      const url=new URL(u);
      const names={'raw.githubusercontent.com':'游戏生产表','paldb.cc':'PalDB','palworld.wiki.gg':'Palworld Wiki','github.com':'Pal Atlas 点位','palsphere.app':'PalSphere'};
      const name=names[url.hostname]||url.hostname;
      return '<a href="'+esc(url.href)+'" target="_blank" rel="noopener noreferrer">'+name+'</a>';
    }).join('')+'</footer>';
  }
  function profile(name,tag,id){
    return '<header class="rg-profile">'+icon(id)+'<div><small>'+esc(tag)+'</small><h2>'+esc(name)+'</h2></div></header>';
  }
  function gatheringDetails(r){
    const related=[item(r.id,r.name+' · 物品图鉴')];
    for(const station of data.production.filter(s=>s.item===r.id))related.push(link(href(base.production,station.id),station.name+' · 据点生产'));
    const tools=(r.tools||[]).map(([id,name])=>item(id,name));
    if(r.pal)tools.push(link(href('#details/partner',r.pal[0]),r.pal[1]+' · 伙伴技能'));
    const pools=(r.pools||[]).flatMap(key=>data.pools[key]);
    const sites=pools.map(p=>{
      const destinations=p.samples.map(([id,x,y])=>link(href('#map/point',id),'('+x+'，'+y+')'));
      if(p.href)destinations.unshift(link(p.href,'全部 '+num(p.count)+' 处'));
      return '<li><b>'+esc(p.label)+'</b>'+links(destinations)+'</li>';
    });
    let locations=sites.length?'<ul class="rg-sites">'+sites.join('')+'</ul>':'';
    if(r.areaLink)locations+=links([link(...r.areaLink)]);
    return profile(r.name,r.group,r.id)+'<p class="rg-note">'+esc(r.note)+'</p>'+
      (r.minutes?stats([['基础刷新间隔',r.minutes+' 分钟']]):'')+
      section('采集工具与条件',tools.length?links(tools):'')+section('地图位置',locations)+section('关联条目',links(related))+citation(r.sources);
  }
  function productionDetails(r){
    const facts=[['基础工作量',num(r.work)]];
    if(r.level)facts.push(['解锁科技','Lv. '+r.level]);
    for(const [job,level] of r.jobs||[])facts.push(['所需工作',job+' Lv. '+level]);
    if(r.power>0)facts.push(['消耗电力',num(r.power)+' / 秒']);
    if(r.auto){
      facts.push(['自动工作量',num(r.auto)+' / 秒'],['表内基础周期','约 '+num(r.work/r.auto)+' 秒']);
    }
    const related=[item(r.item,r.itemName+' · 物品图鉴')];
    if(r.facility)related.push(link(href('#work/facilities',r.facility),'设施与建造材料'));
    if(r.jobs?.length)related.push(link('#work/efficiency','计算帕鲁工作效率'));
    if(gatherById.has(r.item))related.push(link(href(base.gathering,r.item),'野外采集位置'));
    if(r.id==='OilPump')related.push(link('#map/category/crude_oil/palpagos','定位原油油田'));
    return profile(r.name,productionGroup(r),r.item)+section('产物',links([item(r.item,r.itemName)]))+
      section('生产条件','<p class="rg-note">'+esc(r.condition)+'</p>'+stats(facts))+
      section('关联条目',links(related))+citation(data.productionSources);
  }
  const views={};
  function setup(tab){
    const gathering=tab==='gathering', records=data[tab], root=document.getElementById(tab+'-view');
    const el=k=>root.querySelector('[data-rg-'+k+']');
    const input=el('query'),filter=el('filter'),list=el('list'),detail=el('detail');
    const groups=new Map(records.map(r=>[r.id,gathering?r.group:productionGroup(r)]));
    const haystack=new Map(records.map(r=>{
      const areas=(r.pools||[]).flatMap(key=>data.pools[key].map(pool=>pool.label));
      return [r.id,normalize([r.id,r.name,groups.get(r.id),r.note||'',r.item||'',r.itemName||'',...(r.aliases||[]),...areas].join(' '))];
    }));
    for(const label of new Set(groups.values()))filter.add(new Option(label,label));
    let selected,shown=[];
    function render(id){
      const terms=normalize(input.value).split(/\s+/).filter(Boolean);
      shown=records.filter(r=>(!filter.value||groups.get(r.id)===filter.value)&&terms.every(word=>haystack.get(r.id).includes(word)));
      selected=shown.find(r=>r.id===id)||shown[0];
      el('count').textContent=shown.length+(gathering?' 项采集物':' 项生产记录');
      list.innerHTML=shown.map(r=>'<a class="rg-choice" href="'+href(base[tab],r.id)+'"'+(r===selected?' aria-current="true"':'')+'>'+icon(gathering?r.id:r.item)+'<span>'+esc(r.name)+'<small>'+esc(gathering?groups.get(r.id):'产出 · '+r.itemName)+'</small></span></a>').join('')||'<p class="rg-note">没有匹配条目。</p>';
      detail.hidden=!selected;
      detail.innerHTML=selected?(gathering?gatheringDetails(selected):productionDetails(selected)):'';
    }
    function search(){
      render(selected?.id);list.scrollTop=0;
      history.replaceState(history.state,'',selected?href(base[tab],selected.id):base[tab]);
    }
    input.addEventListener('input',search);filter.addEventListener('change',search);
    list.addEventListener('click',e=>{
      if(!e.target.closest('a')||e.ctrlKey||e.metaKey||e.altKey||e.shiftKey||!matchMedia('(max-width:820px)').matches)return;
      requestAnimationFrame(()=>{detail.scrollIntoView({block:'start'});detail.focus({preventScroll:true});});
    });
    return id=>{
      if(id&&groups.has(id)&&!shown.some(r=>r.id===id)){input.value='';filter.value='';}
      render(id||selected?.id);
    };
  }
  return {show(tab,id){views[tab]??=setup(tab);views[tab](id);}};
}

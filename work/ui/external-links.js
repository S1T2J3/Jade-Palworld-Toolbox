function buildExternalDirectory({catalog,facilities,journals,ranch,capture,species,releaseUrl}) {
  // Keep separate articles pointing at one page (e.g. item rarities) searchable
  // by every source name and ID, but only display each URL once per category.
  const groups=new Map();
  const paldb='https://paldb.cc/cn/';
  const palNames=new Map(species.map(p=>[p.key,p.zh]));
  const store=(section,group,href,label,context='',keywords='')=>{
    const url=new URL(href);
    if(url.protocol!=='https:')return;
    if(!groups.has(group))groups.set(group,{section,name:group,entries:new Map()});
    const entries=groups.get(group).entries;
    if(!entries.has(url.href))entries.set(url.href,{url:url.href,host:url.hostname,aliases:new Map()});
    const aliases=entries.get(url.href).aliases;
    const key=[label,context,keywords].join(' ');
    if(!aliases.has(key))aliases.set(key,{label,context,search:key.normalize('NFKC').toLowerCase()});
  };
  const guide='获取与战斗资料',optional='在线词条对照';
  for(const item of catalog.items) {
    if(item.slug)store(optional,'物品资料',paldb+item.slug,item.name,item.id,item.en);
  }
  for(const id of capture.raidEggOnly)if(!capture.enemiesByPal?.[id]?.length)
    store(guide,'团体战攻略','https://palpedia.com/guides/raid-bosses','团体战头目攻略','召唤、胜利奖励及孵化',palNames.get(id)+' '+id);
  for(const row of journals)store(optional,'手记原文',row.source,row.title,row.group,row.id);
  for(const f of facilities)store(optional,'设施资料',paldb+encodeURIComponent(f.id),f.name,f.group,f.id);
  for(const [key,pal] of Object.entries(ranch))
    store(optional,'牧场逐级产出表',pal.source,palNames.get(key)||key,'放牧产物',key);
  if(releaseUrl)store('软件更新','工具箱发布页',releaseUrl,'Jade 幻兽帕鲁工具箱','有仓库访问权限的账号可查看','GitHub 公告 下载 更新');
  const order=['团体战攻略','物品资料','设施资料','牧场逐级产出表','手记原文','工具箱发布页'];
  const collator=new Intl.Collator('zh-CN',{numeric:true});
  return order.filter(name=>groups.has(name)).map(name=>{
    const {section,entries}=groups.get(name);
    const rows=Array.from(entries.values(),entry=>({...entry,aliases:[...entry.aliases.values()]}));
    rows.sort((a,b)=>collator.compare(a.aliases[0].label,b.aliases[0].label));
    return {section,name,rows};
  });
}

function initExternalDirectory(footer,groups) {
  const query=footer.querySelector('[data-external-query]');
  const results=footer.querySelector('[data-external-results]');
  const count=footer.querySelector('[data-external-count]');
  const tools=footer.querySelector('[data-external-tools]');
  const expanded=new Set();
  function render() {
    const term=query.value.normalize('NFKC').trim().toLowerCase();
    const tokens=term.split(/\s+/).filter(Boolean);
    results.replaceChildren();
    let matched=0,lastSection='';
    for(const group of groups) {
      const entries=[];
      for(const entry of group.rows) {
        const common=(group.name+' '+entry.host).toLowerCase();
        const found=!term?entry.aliases[0]:entry.aliases.find(a=>tokens.every(t=>a.search.includes(t)||common.includes(t)));
        if(found)entries.push({entry,found});
      }
      if(!entries.length)continue;
      matched+=entries.length;
      if(group.section!==lastSection){
        const heading=document.createElement('h2');heading.textContent=group.section;
        results.append(heading);lastSection=group.section;
      }
      const fold=document.createElement('details'),summary=document.createElement('summary'),badge=document.createElement('span');
      fold.className='external-group';summary.textContent=group.name;badge.textContent=entries.length.toLocaleString('zh-CN')+' 个链接';
      summary.append(badge);fold.append(summary);results.append(fold);
      const grid=document.createElement('div'),more=document.createElement('button');
      grid.className='external-grid';more.type='button';
      let shown=0;
      function append() {
        const batch=entries.slice(shown,shown+48);
        for(const {entry,found} of batch) {
          const a=document.createElement('a'),detail=document.createElement('small');
          a.href=entry.url;a.target='_blank';a.rel='noopener noreferrer';a.textContent=found.label+' ↗';
          detail.textContent=found.context+' · '+entry.host;
          a.append(detail);grid.append(a);
        }
        shown+=batch.length;
        more.hidden=shown===entries.length;
        more.textContent='显示更多（'+shown+' / '+entries.length+'）';
      }
      function open() {
        if(shown)return;
        fold.append(grid,more);append();
      }
      more.addEventListener('click',append);
      fold.open=!!term||expanded.has(group.name);
      if(fold.open)open();
      fold.addEventListener('toggle',()=>{
        if(fold.open)open();
        if(!term){if(fold.open)expanded.add(group.name);else expanded.delete(group.name);}
      });
    }
    count.textContent=matched?(term?'找到 ':'共 ')+matched.toLocaleString('zh-CN')+' 个分组链接':'没有匹配的外部链接。';
  }
  query.addEventListener('input',render);
  tools.hidden=false;render();
}

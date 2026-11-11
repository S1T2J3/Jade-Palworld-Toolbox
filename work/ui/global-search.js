function prepareAtlasSearch(entries) {
  const collator=new Intl.Collator('zh-CN',{numeric:true});
  const normalize=value=>String(value||'').normalize('NFKC').toLowerCase().replace(/[\s_-]+/g,'');
  const rows=entries.map(entry=>({...entry,searchName:normalize(entry.name),searchAliases:(entry.aliases||[]).map(normalize)}));
  return function search(query,kind='全部') {
    const q=normalize(query);
    if(!q) return [];
    return rows.filter(row=>kind==='全部'||row.kind===kind).map(row=>{
      const name=row.searchName,aliases=row.searchAliases;
      const score=name===q?1000:name.startsWith(q)?800:name.includes(q)?650:
        aliases.some(s=>s===q)?600:aliases.some(s=>s.startsWith(q))?500:aliases.some(s=>s.includes(q))?350:0;
      return {row,score};
    }).filter(hit=>hit.score).sort((a,b)=>b.score-a.score||collator.compare(a.row.name,b.row.name))
      .map(hit=>hit.row);
  };
}

function initGlobalSearch({entries}) {
  const el=id=>document.getElementById('atlas-search-'+id);
  const dialog=el('dialog'),input=el('input'),results=el('results'),summary=el('status'),more=el('more');
  const search=prepareAtlasSearch(entries),counts=new Map();
  for(const row of entries) counts.set(row.kind,(counts.get(row.kind)||0)+1);
  let kind='全部',limit=80,hits=[],timer;
  const fragment=document.createDocumentFragment();
  for(const [name,count] of [['全部',entries.length],...counts]) {
    const button=document.createElement('button');button.type='button';button.dataset.searchKind=name;
    button.textContent=name+' '+count;button.setAttribute('aria-pressed',String(name===kind));fragment.append(button);
  }
  el('filters').replaceChildren(fragment);
  function render() {
    hits=search(input.value,kind);
    const active=!!input.value.trim();
    summary.textContent=active?(hits.length?'找到 '+hits.length+' 条资料，显示 '+Math.min(limit,hits.length)+' 条。':'没有匹配的资料，试试其他名称、英文名或游戏编号。'):
      '可搜索 '+entries.length.toLocaleString('zh-CN')+' 条已有资料。输入名称、英文名或游戏编号。';
    const fragment=document.createDocumentFragment();
    for(const row of hits.slice(0,limit)) {
      const link=document.createElement('a');link.href=row.href;link.className='atlas-search-result';
      const badge=document.createElement('small');badge.textContent=row.kind;
      const text=document.createElement('span'),title=document.createElement('strong');title.textContent=row.name;
      const hint=document.createElement('span');hint.className='atlas-search-hint';hint.textContent=row.hint||row.aliases?.[0]||'';
      text.append(title,hint);link.append(badge,text);fragment.append(link);
    }
    results.replaceChildren(fragment);more.hidden=hits.length<=limit;
  }
  function update(){clearTimeout(timer);limit=80;results.scrollTop=0;render();}
  input.addEventListener('input',event=>{if(!event.isComposing){clearTimeout(timer);timer=setTimeout(update,90);}});
  input.addEventListener('compositionend',update);
  input.addEventListener('keydown',event=>{
    if(event.isComposing) return;
    if(event.key==='ArrowDown'||event.key==='Enter') {
      update();const first=results.querySelector('a');
      if(first){event.preventDefault();if(event.key==='Enter')first.click();else first.focus();}
    }
  });
  el('filters').addEventListener('click',event=>{
    const button=event.target.closest('[data-search-kind]');if(!button)return;
    kind=button.dataset.searchKind;
    for(const item of el('filters').children)item.setAttribute('aria-pressed',String(item===button));
    update();
  });
  results.addEventListener('keydown',event=>{
    if(!['ArrowDown','ArrowUp','Home','End'].includes(event.key))return;
    const links=[...results.querySelectorAll('a')],index=links.indexOf(document.activeElement);
    if(index<0)return;event.preventDefault();
    const next=event.key==='Home'?0:event.key==='End'?links.length-1:event.key==='ArrowDown'?index+1:index-1;
    if(next<0)input.focus();else links[Math.min(next,links.length-1)]?.focus();
  });
  results.addEventListener('click',event=>{if(event.target.closest('a'))dialog.close();});
  more.addEventListener('click',()=>{const index=limit;limit+=80;render();results.querySelectorAll('a')[index]?.focus();});
  dialog.addEventListener('close',()=>clearTimeout(timer));
  render();
  return {focus:()=>input.focus(),refresh:update};
}

if(typeof module!=='undefined'&&module.exports)module.exports={prepareAtlasSearch};

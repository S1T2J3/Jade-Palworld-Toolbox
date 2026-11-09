function initJournals({data,escape}) {
  const el=id=>document.getElementById('jn-'+id);
  const {catalog}=data;
  const byId=new Map(catalog.map(r=>[r.id,r])),byPoint=new Map(catalog.map(r=>[r.pointId,r]));
  const groups=[...new Set(catalog.map(r=>r.group))],mapNames={palpagos:'帕洛斯群岛',worldtree:'世界树'};
  for(const group of groups)el('group').add(new Option(group,group));
  const route=id=>'#guide/journals/'+encodeURIComponent(id);
  const clean=s=>s.normalize('NFKC').toLowerCase().replace(/\s+/g,'');
  const searchText=new Map(catalog.map(r=>[r.id,clean(r.title+r.group+r.summary+r.body)]));
  function list() {
    const group=el('group').value,map=el('region').value,q=clean(el('search').value);
    const found=catalog.filter(r=>(!group||r.group===group)&&(!map||r.map===map)&&(!q||searchText.get(r.id).includes(q)));
    el('count').textContent=found.length===catalog.length?'共 '+catalog.length+' 篇':found.length+' / '+catalog.length+' 篇';
    el('list').innerHTML=groups.map(g=>{
      const rows=found.filter(r=>r.group===g);if(!rows.length)return '';
      return '<section class="jn-group"><h2>'+escape(g)+'</h2><div class="jn-grid">'+rows.map(r=>
        '<a class="jn-card" data-journal="'+escape(r.id)+'" href="'+route(r.id)+'"><strong>'+escape(r.title)+'</strong><small>'+mapNames[r.map]+' · '+r.x+'，'+r.y+'</small></a>'
      ).join('')+'</div></section>';
    }).join('')||'<p>没有找到符合条件的手记。</p>';
  }
  for(const id of ['group','region'])el(id).addEventListener('change',list);
  el('search').addEventListener('input',list);
  return {show(id){
    const selected=byId.get(id)||byPoint.get(id);
    el('index').hidden=!!selected;el('reader').hidden=!selected;
    if(!selected){list();return;}
    if(id===selected.pointId)history.replaceState(history.state,'',route(selected.id));
    const idx=catalog.indexOf(selected);
    el('count').textContent=(idx+1)+' / '+catalog.length;
    el('series').textContent=selected.group;el('name').textContent=selected.title;
    el('summary').textContent=selected.summary;el('source').href=selected.source;
    el('map').href='#map/point/'+encodeURIComponent(selected.pointId);
    el('map').textContent='地图定位 · '+mapNames[selected.map]+'（'+selected.x+'，'+selected.y+'） ↗';
    for(const [key,n] of [['prev',idx-1],['next',idx+1]]){
      const a=el(key),row=catalog[n];a.hidden=!row;
      if(row){a.href=route(row.id);a.title=row.title;}else a.removeAttribute('href');
    }
    el('text').replaceChildren(...selected.body.split(/\n\s*\n/g).map(paragraph=>{
      const p=document.createElement('p');p.textContent=paragraph;return p;
    }));
  }};
}

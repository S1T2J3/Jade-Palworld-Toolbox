function initSpawnMap({data,mapData,images,species,portraits,avatar,dex,typeSet,normalize,matchScore,onSpeciesChange}) {
  const $=id=>document.getElementById(id);
  const viewport=$('sm-viewport'), layer=$('sm-markers'), sheet=$('sm-sheet'), hover=$('sm-hover'), detail=$('sm-detail');
  const list=$('sm-list'), input=$('sm-search'), palette=$('sm-species'), regionSelect=$('sm-region'), timeSelect=$('sm-time'), kindSelect=$('sm-kind');
  const esc=value=>String(value??'').replace(/[&<>"']/g,ch=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[ch]));
  const byKey=new Map(species.map((p,i)=>[p.key,i]));
  const regions=new Map(mapData.maps.map(m=>[m.id,m]));
  const candidates=species.map((_,i)=>i);
  const siteMap=new Map(), pins=new Map(), pageSize=45;
  const timeName=code=>['昼夜均可','仅夜间','时段未标注'][code] || '时段未标注';
  const kindName=code=>code===1?'头目':'野外';
  const fmt=value=>Number(value).toLocaleString('zh-CN');
  let index=byKey.get('SheepBall')??0, region=data.mapIds[0], located=[], shown=[], selected=null;
  let groups=new Map(), shownById=new Map(), focusKey=null, hovered=null, next=pageSize, choices=[], active=0;
  const pal=()=>species[index];
  function levelText(record) {
    return Number.isFinite(record.lo)&&Number.isFinite(record.hi) ?
      (record.lo===record.hi?'Lv. '+record.lo:'Lv. '+record.lo+'–'+record.hi) : '等级待核实';
  }
  function levelSummary(site) {
    return [...new Set(site.records.map(levelText))].join(' / ');
  }
  function tagSummary(site) {
    return [...new Set(site.records.map(r=>kindName(r.kind)+' · '+timeName(r.time)))].join(' / ');
  }
  function rangeLines(site) {
    return site.records.map(r=>levelText(r)+' · '+kindName(r.kind)+' · '+timeName(r.time));
  }
  function coordinates(site) {
    return Number.isFinite(site.x)&&Number.isFinite(site.y)?site.x+'，'+site.y:'';
  }
  function hidePalette() {
    palette.hidden=true;input.setAttribute('aria-expanded','false');input.removeAttribute('aria-activedescendant');
  }
  function highlight() {
    const buttons=palette.querySelectorAll('[data-sm-species]');
    buttons.forEach((el,i)=>{el.dataset.active=String(i===active);el.setAttribute('aria-selected',String(i===active));});
    if(buttons[active]){
      const el=buttons[active];input.setAttribute('aria-activedescendant',el.id);
      if(el.offsetTop<palette.scrollTop) palette.scrollTop=el.offsetTop;
      if(el.offsetTop+el.offsetHeight>palette.scrollTop+palette.clientHeight) palette.scrollTop=el.offsetTop+el.offsetHeight-palette.clientHeight;
    }else input.removeAttribute('aria-activedescendant');
  }
  function showPalette() {
    const q=normalize(input.value.trim()===pal().zh?'':input.value);
    choices=q ? candidates.map(i=>({i,s:matchScore(i,q)})).filter(m=>m.s).sort((a,b)=>b.s-a.s||a.i-b.i).map(m=>m.i) : candidates;
    active=0;
    palette.innerHTML=choices.length ? choices.map(i=>{
      const p=species[i];return `<button type="button" tabindex="-1" role="option" data-sm-species="${esc(p.key)}" id="sm-option-${i}">${avatar(p)}<b>${esc(p.zh)}</b><small>${esc(dex(p))}</small></button>`;
    }).join(''):'<p class="sm-no-match">没有找到匹配的帕鲁。</p>';
    palette.hidden=false;input.setAttribute('aria-expanded','true');palette.scrollTop=0;highlight();
  }
  input.addEventListener('focus',()=>{input.select();showPalette();});
  input.addEventListener('click',()=>{if(palette.hidden) showPalette();});
  input.addEventListener('input',e=>{if(!e.isComposing)showPalette();});
  input.addEventListener('compositionend',showPalette);
  input.addEventListener('blur',()=>{hidePalette();input.value=pal().zh;});
  input.addEventListener('keydown',e=>{
    if(e.isComposing||e.keyCode===229)return;
    if(e.key==='Escape'){hidePalette();input.value=pal().zh;e.preventDefault();return;}
    if(e.key==='ArrowDown'||e.key==='ArrowUp'){
      e.preventDefault();if(palette.hidden){showPalette();return;}
      active=Math.min(Math.max(0,choices.length-1),Math.max(0,active+(e.key==='ArrowDown'?1:-1)));highlight();
    }
    if(e.key==='Enter'&&!palette.hidden){e.preventDefault();if(choices[active]!=null) choose(species[choices[active]].key,true);}
  });
  palette.addEventListener('pointerdown',e=>{if(e.pointerType==='mouse')e.preventDefault();});
  // Preserve the editable combobox's focus for touch-generated mouse events.
  palette.addEventListener('mousedown',e=>e.preventDefault());
  palette.addEventListener('click',e=>{const el=e.target.closest('[data-sm-species]');if(el)choose(el.dataset.smSpecies,true);});
  for(const id of data.mapIds) regionSelect.add(new Option(regions.get(id).name,id));
  function loadSites(key) {
    siteMap.clear();
    for(const [id,lo,hi,time,kind] of data.bySpecies[key]||[]) {
      const pos=data.sites[id];
      if(!pos) continue;
      if(!siteMap.has(id)) siteMap.set(id,{id,region:data.mapIds[pos[0]],u:pos[1],v:pos[2],x:pos[3],y:pos[4],records:[]});
      siteMap.get(id).records.push({lo,hi,time,kind});
    }
    located=[...siteMap.values()];
  }
  function renderRegionCounts() {
    for(const option of regionSelect.options) {
      const count=located.filter(p=>p.region===option.value).length;
      option.textContent=regions.get(option.value).name+'（'+fmt(count)+'）';
    }
    regionSelect.value=region;
  }
  function drawTerrain() {
    if(viewport.dataset.map!==region){sheet.src=images[region];sheet.alt=regions.get(region).name+'地形图';viewport.dataset.map=region;}
  }
  function choose(key,notify=false) {
    if(!byKey.has(key))return false;
    index=byKey.get(key);input.value=pal().zh;hidePalette();
    loadSites(key);
    if(located.length&&!located.some(p=>p.region===region))region=located[0].region;
    timeSelect.value='all';kindSelect.value='all';
    drawTerrain();renderRegionCounts();renderProfile();camera.fit();applyFilters();
    if(notify) onSpeciesChange?.(key);
    return true;
  }
  function renderProfile() {
    const p=pal(), facts=data.profiles?.[p.key]||{};
    const stats=[['生命',facts.hp],['攻击',facts.attack],['防御',facts.defense]].filter(pair=>Number.isFinite(pair[1]));
    const statMarkup=stats.length?'<h3 class="sm-fact-heading">种族值</h3><dl class="sm-stats">'+stats.map(([s,v])=>'<div><dt>'+s+'</dt><dd>'+v+'</dd></div>').join('')+'</dl>':'';
    const key=encodeURIComponent(p.key);
    const work=facts.work?.length?'<h3 class="sm-fact-heading">工作适应性</h3><div class="sm-skills">'+facts.work.map(([w,lv])=>'<a href="#work/efficiency/'+key+'/'+encodeURIComponent(w)+'">'+esc(w)+' '+esc(lv)+'</a>').join('')+'</div>':'';
    const partner=facts.partner?'<div class="sm-partner"><span>伙伴技能</span><a href="#details/partner/'+key+'"><b>'+esc(facts.partner)+'</b></a></div>':'';
    $('sm-profile').innerHTML=`<div class="sm-profile-heading">${avatar(p)}<div><h2 id="sm-profile-name">${esc(p.zh)}</h2><p>${esc(dex(p))} · ${esc(p.en)}</p><div>${typeSet(p)}</div></div></div><nav class="sm-profile-tools" aria-label="帕鲁资料与计算"><a href="#paldex/${key}">查看帕鲁图鉴</a><a href="#details/capture/${key}">计算捕捉概率</a><a href="#details/stats/${key}">计算属性值</a></nav>${partner}${statMarkup}${work}`;
  }
  function dismiss() {
    selected=null;detail.hidden=true;hideHover();
    list.querySelector('[aria-pressed="true"]')?.setAttribute('aria-pressed','false');
    camera.refresh();
  }
  function applyFilters() {
    dismiss();list.scrollTop=0;next=pageSize;
    const inRegion=located.filter(site=>site.region===region);
    shown=inRegion.map(site=>{
      const records=site.records.filter(r=>{
        const timeMatches=timeSelect.value==='all' || (timeSelect.value==='day' && r.time===0) ||
          (timeSelect.value==='night' && (r.time===0 || r.time===1)) || (timeSelect.value==='nightOnly' && r.time===1);
        return timeMatches && (kindSelect.value==='all'||r.kind===({wild:0,alpha:1})[kindSelect.value]);
      });
      return {...site,records,category:records.some(r=>r.kind===1)?'alpha':'wild'};
    }).filter(p=>p.records.length);
    shownById=new Map(shown.map(p=>[p.id,p]));
    $('sm-count').textContent=fmt(shown.length)+' 处';
    $('sm-summary').textContent=pal().zh+' · '+regions.get(region).name+' · '+fmt(inRegion.length)+' 处位置';
    renderList();camera.refresh();
  }
  regionSelect.addEventListener('change',()=>{region=regionSelect.value;drawTerrain();camera.fit();applyFilters();});
  timeSelect.addEventListener('change',applyFilters);kindSelect.addEventListener('change',applyFilters);
  function renderList() {
    if(!shown.length) {
      let reason='当前筛选下没有刷新记录。<button type="button" data-sm-reset>清除筛选</button>';
      if(!located.length)reason='本版地图未收录这只帕鲁的野外或头目刷新位置。';
      else if(!located.some(site=>site.region===region)){
        const other=located[0].region;
        reason=`当前区域没有此帕鲁的刷新记录。<button type="button" data-sm-switch="${esc(other)}">查看${esc(regions.get(other).name)}</button>`;
      }
      list.innerHTML='<div class="sm-empty">'+reason+'</div>';return;
    }
    list.innerHTML=shown.slice(0,next).map(site=>`<button class="sm-site" type="button" data-sm-result="${site.id}" aria-pressed="${selected===site}"><span class="sm-site-mark ${site.category==='alpha'?'sm-alpha':''}" aria-hidden="true"></span><span class="sm-site-copy"><strong>${esc(coordinates(site))}</strong><small>${esc(tagSummary(site))}</small></span><span class="sm-site-level">${esc(levelSummary(site))}</span></button>`).join('')+
      (next<shown.length?`<button type="button" class="wm-more" data-sm-more>显示更多（${fmt(next)} / ${fmt(shown.length)}）</button>`:'');
  }
  function selectSite(site,center) {
    if(!site)return;
    hideHover();selected=site;
    list.querySelector('[aria-pressed="true"]')?.setAttribute('aria-pressed','false');
    list.querySelector('[data-sm-result="'+site.id+'"]')?.setAttribute('aria-pressed','true');
    $('sm-detail-content').innerHTML='<h3>'+esc(pal().zh)+'</h3><p>'+esc(regions.get(region).coordLabel)+'　'+esc(coordinates(site))+'</p>'+
      rangeLines(site).map(line=>'<p>'+esc(line)+'</p>').join('')+
      (coordinates(site)?'<a class="wm-spawn-link" href="#map/coords/'+encodeURIComponent(site.region)+'/'+site.x+'/'+site.y+'">在世界地图上定位 ↗</a>':'');
    detail.hidden=false;
    if(center) {
      camera.center(site.u,site.v,4);
      if(matchMedia('(max-width:760px)').matches)viewport.scrollIntoView({block:'nearest'});
    }
    camera.refresh();
  }
  list.addEventListener('click',e=>{
    const row=e.target.closest('[data-sm-result]');if(row){selectSite(shownById.get(Number(row.dataset.smResult)),true);return;}
    if(e.target.closest('[data-sm-more]')){const first=shown[next];next+=pageSize;renderList();list.querySelector('[data-sm-result="'+first.id+'"]')?.focus({preventScroll:true});return;}
    if(e.target.closest('[data-sm-reset]')){timeSelect.value='all';kindSelect.value='all';applyFilters();return;}
    const other=e.target.closest('[data-sm-switch]');
    if(other){regionSelect.value=other.dataset.smSwitch;regionSelect.dispatchEvent(new Event('change'));}
  });
  function hideHover() {if(hovered){hover.hidden=true;hovered.removeAttribute('aria-describedby');hovered=null;}}
  function positionHover(marker) {
    const box=viewport.getBoundingClientRect(),pin=marker.getBoundingClientRect();
    let left=pin.left+pin.width/2-box.left-hover.offsetWidth/2;
    let top=pin.top-box.top-hover.offsetHeight-7;
    if(top<4)top=pin.bottom-box.top+7;
    hover.style.left=Math.max(4,Math.min(box.width-hover.offsetWidth-4,left))+'px';
    hover.style.top=Math.max(4,Math.min(box.height-hover.offsetHeight-4,top))+'px';
  }
  function showHover(marker) {
    if(!marker||viewport.classList.contains('wm-panning'))return;
    const site=shownById.get(Number(marker.dataset.smPoint));
    const group=groups.get(marker.dataset.smCluster);
    if(!site&&!group)return;
    if(hovered!==marker) hovered?.removeAttribute('aria-describedby');
    hovered=marker;marker.setAttribute('aria-describedby','sm-hover');
    hover.innerHTML=site ? '<b>'+esc(pal().zh)+'</b>'+rangeLines(site).map(s=>'<span>'+esc(s)+'</span>').join('') :
      '<b>'+group.points.length+' 处刷新位置</b><span>点按展开以查看各处等级。</span>';
    hover.hidden=false;positionHover(marker);
  }
  const pinSelector='[data-sm-point],[data-sm-cluster]';
  function drawPins(g) {
    g.panComplete=shown.length<=160;
    const clusters=clusterMapPoints(shown,g,camera.zoom()<2?25:21,selected);
    const keep=new Set();groups=new Map();
    for(const group of clusters) {
      const first=group.points[0],n=group.points.length,multi=n>1;
      const key=(multi?'c':'s')+first.id;keep.add(key);
      if(multi)groups.set(key,group);
      let pin=pins.get(key);
      if(pin && g.panOnly)continue;
      if(!pin){
        pin=document.createElement('button');pin.type='button';pin.tabIndex=-1;
        if(multi){pin.dataset.smCluster=key;}
        else pin.dataset.smPoint=String(first.id);
        pins.set(key,pin);layer.append(pin);
      }
      pin.classList.toggle('sm-cluster',multi);pin.classList.toggle('sm-alpha',group.category==='alpha');pin.classList.toggle('sm-pinned',first===selected&&!multi);
      const bossPic=group.category==='alpha' && portraits?.[pal().en];
      const contentId=bossPic?'alpha:'+pal().key+':'+n:multi?'count:'+n:'dot';
      if(pin.dataset.content!==contentId){
        pin.dataset.content=contentId;pin.classList.toggle('sm-boss-picture',Boolean(bossPic));pin.replaceChildren();
        if(bossPic){
          const img=new Image();img.alt='';img.src=bossPic;img.draggable=false;pin.append(img);
          if(multi){const badge=document.createElement('span');badge.className='sm-boss-count';badge.textContent=String(n);pin.append(badge);}
        }else if(multi)pin.textContent=String(n);
        else pin.innerHTML='<span class="sm-pin-core" aria-hidden="true"></span>';
      }
      if(multi){pin.setAttribute('aria-label',n+' 处刷新位置，展开');}
      else pin.setAttribute('aria-label',pal().zh+' '+coordinates(first)+' '+rangeLines(first).join('；'));
      pin.style.left=group.x+'px';pin.style.top=group.y+'px';
    }
    for(const [key,pin] of pins) if(!keep.has(key)){if(hovered===pin)hideHover();pin.remove();pins.delete(key);}
    if(!keep.has(focusKey))focusKey=keep.values().next().value;
    for(const [key,pin] of pins){const next=key===focusKey?0:-1;if(pin.tabIndex!==next)pin.tabIndex=next;}
    if(hovered) {if(viewport.classList.contains('wm-panning'))hideHover();else showHover(hovered);}
  }
  layer.addEventListener('pointerover',e=>{if(e.pointerType!=='touch')showHover(e.target.closest(pinSelector));});
  layer.addEventListener('pointerout',e=>{if(e.pointerType!=='touch'&&e.target.closest(pinSelector)!==e.relatedTarget?.closest?.(pinSelector))hideHover();});
  layer.addEventListener('focusin',e=>{
    const pin=e.target.closest(pinSelector);if(!pin)return;
    focusKey=pin.dataset.smCluster||'s'+pin.dataset.smPoint;for(const [key,el] of pins)el.tabIndex=key===focusKey?0:-1;
    showHover(pin);
  });
  layer.addEventListener('focusout',hideHover);
  layer.addEventListener('keydown',e=>{
    const important=['ArrowLeft','ArrowUp','ArrowRight','ArrowDown','Home','End'];
    if(!important.includes(e.key))return;
    const buttons=camera.visibleMarkers(),at=buttons.indexOf(e.target);if(at<0)return;
    e.preventDefault();
    const dest=e.key==='Home'?0:e.key==='End'?buttons.length-1:Math.max(0,Math.min(buttons.length-1,at+(['ArrowLeft','ArrowUp'].includes(e.key)?-1:1)));
    buttons[dest]?.focus({preventScroll:true});
  });
  function activatePin(pin) {
    if(pin.hasAttribute('data-sm-point')) {selectSite(shownById.get(Number(pin.dataset.smPoint)),false);return;}
    const group=groups.get(pin.dataset.smCluster);if(!group)return;
    const points=group.points,first=points[0];
    const separated=points.some(p=>Math.abs(p.u-first.u)+Math.abs(p.v-first.v)>.00005);
    dismiss();
    if(separated&&camera.zoom()<15.99) {
      camera.center(points.reduce((s,p)=>s+p.u,0)/points.length,points.reduce((s,p)=>s+p.v,0)/points.length,camera.zoom()*2);
      return;
    }
    $('sm-detail-content').innerHTML='<h3>附近的刷新位置</h3><div class="sm-cluster-list">'+points.map(p=>`<button type="button" data-sm-pick="${p.id}">${esc(coordinates(p))} · ${esc(levelSummary(p))}</button>`).join('')+'</div>';
    detail.hidden=false;
  }
  detail.addEventListener('click',e=>{const b=e.target.closest('[data-sm-pick]');if(b)selectSite(shownById.get(Number(b.dataset.smPick)),true);});
  $('sm-close-detail').addEventListener('click',()=>{dismiss();viewport.focus({preventScroll:true});});
  const camera=createMapCamera({viewport,sheet,markerLayer:layer,onMove:hideHover,zoomLabel:$('sm-zoom-level'),zoomIn:$('sm-zoom-in'),zoomOut:$('sm-zoom-out'),fitButton:$('sm-fit'),
    onDraw:drawPins,onBlank:dismiss,onPoint:activatePin,uiSelector:'[data-sm-ui]',markerSelector:pinSelector});
  const sourceLinks=(data.sources||[]).filter(s=>/^https?:\/\//.test(s.url)).map(s=>`<a href="${esc(s.url)}" target="_blank" rel="noopener noreferrer">${esc(s.name)}</a>`);
  $('sm-source').innerHTML='野外标记为刷新区域中心，帕鲁可能在周围出现。头目标记为资料收录的头目位置。'+(sourceLinks.length?'　资料　'+sourceLinks.join(' · '):'');
  choose(pal().key);
  return {selectSpecies:key=>key===pal().key || choose(key),refresh:camera.refresh,deactivate:()=>{hidePalette();hideHover();camera.deactivate();}};
}

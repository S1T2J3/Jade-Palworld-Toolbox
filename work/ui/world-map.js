function initWorldMap({data,images,species,portraits,worlds}) {
  const $=id=>document.getElementById(id);
  const viewport=$('wm-viewport'), sheet=$('wm-sheet'), markerLayer=$('wm-markers');
  const list=$('wm-list'), categoriesBox=$('wm-categories'), search=$('wm-search');
  const detail=$('wm-detail'), mapSelect=$('wm-map-select'), sources=$('wm-source');
  const esc=value=>String(value??'').replace(/[&<>"']/g,char=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[char]));
  const normalize=s=>String(s??'').normalize('NFKC').trim().toLowerCase();
  const fmt=new Intl.NumberFormat('zh-CN').format;
  const compareNames=new Intl.Collator('zh-CN').compare;
  const categories=[...data.categories,{id:'custom',name:'我的点位',color:'#30735e',default:true}];
  const categoryById=new Map(categories.map(c=>[c.id,c]));
  const mapById=new Map(data.maps.map(m=>[m.id,m]));
  const byMap=new Map(data.maps.map(m=>[m.id,[]])), pointById=new Map();
  const categoryCounts=new Map(), categoryColors=new Map();
  const mineralIcons=images.minerals;
  const {defaults:defaultIcons,art:mapIcons}=images.mapIcons;
  const palByKey=new Map(species.map(p=>[p.key,p]));
  const mineralColors={ore:'#a96036',coal:'#45474b',sulfur:'#a6810b',pure_quartz:'#4b91b0',hexolite_quartz:'#a852a0',
    chromite:'#63569e',soralite:'#c66525',paloxite:'#338365',crude_oil:'#29656b'};
  function icon(category,variant) {
    if(category==='custom') return '<svg class="wm-custom-art" viewBox="0 0 24 24" aria-hidden="true"><path d="M5 21V3m1 1c4-3 7 3 13 0v9c-5 3-9-3-13 0"/></svg>';
    if (mineralIcons[category]) return `<img class="wm-item-art" data-mineral="${esc(category)}" alt="" draggable="false" src="${mineralIcons[category]}">`;
    const key=variant || defaultIcons[category];
    return `<img class="wm-game-art" data-map-icon="${esc(key)}" alt="" draggable="false" src="${mapIcons[key]}">`;
  }
  function portrait(point) {
    return point.category==='alpha_pals' && portraits[palByKey.get(point.palKey)?.en];
  }
  function pointArt(point) {
    const image=portrait(point);
    return image ? `<img class="wm-pal-art" data-pal-key="${esc(point.palKey)}" alt="" draggable="false" src="${image}">` : icon(point.category,point.iconKey);
  }
  function levelLabel(point) {
    if (point.guardLevels) return '守军等级 '+point.guardLevels.join('–');
    if (Number.isFinite(point.level)) return '等级 '+point.level;
    return point.category==='alpha_pals'?'暂缺等级记录':'';
  }
  for (const c of categories) categoryColors.set(c.id,mineralColors[c.id] || (/^#[a-f\d]{6}$/i.test(c.color||'') ? c.color : '#487467'));
  for (const point of data.points) {
    const records=byMap.get(point.map), category=categoryById.get(point.category);
    if (!records || !category || !Number.isFinite(point.u) || !Number.isFinite(point.v)) continue;
    const p={...point,id:String(point.id),search:normalize([point.name,point.detail,point.en,point.nearest,category.name,
      categoryById.get(point.alsoCategory)?.name].filter(Boolean).join(' '))};
    records.push(p);pointById.set(p.id,p);
  }
  for (const [id,points] of byMap) {
    const counts=new Map();
    for (const p of points) {
      counts.set(p.category,(counts.get(p.category)||0)+1);
      if (p.alsoCategory) counts.set(p.alsoCategory,(counts.get(p.alsoCategory)||0)+1);
    }
    categoryCounts.set(id,counts);
    points.sort((a,b)=>compareNames(a.name,b.name)||compareNames(a.id,b.id));
  }
  for (const m of data.maps) mapSelect.add(new Option(m.name,m.id));
  const listChunk=45;
  let map, selected=null, visible=[], elsewhere=[], limit=listChunk, pointLayers=[];
  let outputGroups=new Map(), hovered=null, tabKey=null;
  const chosenByMap=new Map(), livePins=new Map();
  const baseByMap=new Map(byMap), customIds=new Set();
  const progressFilter=$('wm-completion-filter'),pinDialog=$('wm-pin-dialog');
  let doneIds=new Set(),notesWorld=null,draft=null,notesDirty=false;
  const isPointDone=p=>Boolean(p && doneIds.has(p.id));
  let coordinateTarget=null;
  const coordinateMark=document.createElement('span');
  coordinateMark.id='wm-coordinate-target';coordinateMark.hidden=true;
  coordinateMark.setAttribute('aria-hidden','true');markerLayer.append(coordinateMark);
  function clearCoordinate() {coordinateTarget=null;coordinateMark.hidden=true;}

  // The two maps use different in-game coordinate readouts (see build-map-data.cjs).
  const bounds={
    palpagos:{left:(-724400-158000)/459,top:(349400+123888)/459,span:1448800/459},
    worldtree:{left:-127.7,top:648.7+341797/1335.144531,span:341797/1335.144531}
  };
  function coordinatesAt(region,u,v) {
    const b=bounds[region];
    return {x:Math.round(b.left+u*b.span),y:Math.round(b.top-v*b.span)};
  }
  function mapNotesChanged(event) {
    if(event && $('map-view').hidden) {notesDirty=true;return;}
    const changedId=event?.key?.slice('pal-atlas.map.v1.'.length);
    if(!notesDirty && changedId && !changedId.startsWith('custom-') && notesWorld===worlds.storage.worldId) {
      if(worlds.isMapPointDone(changedId)) doneIds.add(changedId);else doneIds.delete(changedId);
      updateFilter(true);return;
    }
    notesDirty=false;
    const state=worlds.mapNotes(),worldId=worlds.storage.worldId;
    if(notesWorld!==worldId) {
      cancelDraft();progressFilter.value='all';$('wm-note-message').hidden=true;
      if(map) dismiss();
    }
    notesWorld=worldId;doneIds=new Set(state.completed);
    for(const id of customIds) pointById.delete(id);
    customIds.clear();
    const customByMap=new Map(data.maps.map(m=>[m.id,[]]));
    for(const pin of state.custom) {
      const p={...pin,...coordinatesAt(pin.map,pin.u,pin.v),detail:pin.note,category:'custom',
        search:normalize([pin.name,pin.note,'我的点位'].join(' '))};
      if(pin.done) doneIds.add(p.id);
      pointById.set(p.id,p);customIds.add(p.id);customByMap.get(pin.map).push(p);
    }
    for(const [region,custom] of customByMap) {
      custom.sort((a,b)=>compareNames(a.name,b.name)||compareNames(a.id,b.id));
      byMap.set(region,custom.concat(baseByMap.get(region)));
      categoryCounts.get(region).set('custom',custom.length);
    }
    if(map) {renderCategories();updateFilter(true);updateSummary();}
  }

  const chosen=()=>chosenByMap.get(map.id);
  const coordinateText=p=>Number.isFinite(p.x)&&Number.isFinite(p.y) ? p.x+'，'+p.y : '';
  const catName=id=>categoryById.get(id)?.name || id;
  const color=id=>categoryColors.get(id);

  function renderCategories() {
    const counts=categoryCounts.get(map.id), enabled=chosen();
    categoriesBox.innerHTML=categories.filter(c=>counts.has(c.id)).map(c=>
      `<label class="wm-kind" style="--wm-color:${color(c.id)}" title="${esc(c.name)}"><input type="checkbox" data-wm-category="${esc(c.id)}" aria-label="${esc(c.name)}" ${enabled.has(c.id)?'checked':''}><span class="wm-kind-icon">${icon(c.id)}</span><span class="wm-kind-name">${esc(c.name)}</span><small>${fmt(counts.get(c.id))}</small></label>`
    ).join('');
    updateExpandControl();
  }
  function updateExpandControl() {
    const button=$('wm-category-expand'), expanded=button.getAttribute('aria-expanded')==='true';
    button.hidden=!expanded && categoriesBox.scrollHeight<=categoriesBox.clientHeight+1;
    button.textContent=expanded?'收起分类':'展开全部 '+categoryCounts.get(map.id).size+' 类';
  }
  function renderList() {
    $('wm-count').textContent=fmt(visible.length)+' 处';
    if (!visible.length) {
      const note=search.value.trim()?'没有找到匹配点位。':(chosen().size?'当前没有点位。':'选择上方类别以查看点位。');
      list.innerHTML=`<div class="wm-empty">${note}${elsewhere.length?'<button type="button" data-wm-reveal>显示其他类别中的 '+fmt(elsewhere.length)+' 处匹配</button>':''}</div>`;
      return;
    }
    list.innerHTML=visible.slice(0,limit).map(p=>{
      const where=coordinateText(p);
      const title=esc(p.name),id=esc(p.id);
      return `<div class="wm-result-row"><button class="wm-result" type="button" data-wm-result="${id}" aria-pressed="${selected?.id===p.id}"><span class="wm-symbol" style="--wm-color:${color(p.category)}">${pointArt(p)}</span><span class="wm-copy"><strong>${title}${isPointDone(p)?'<span class="wm-done-tag">✓ 已完成</span>':''}</strong><small>${esc([p.nearest || catName(p.category),levelLabel(p),where].filter(Boolean).join(' · '))}</small></span></button><label class="wm-row-check" title="标记完成"><input type="checkbox" data-wm-check="${id}" aria-label="${title} 已完成" ${isPointDone(p)?'checked':''}></label></div>`;
    }).join('')+(visible.length>limit?`<button class="wm-more" type="button" id="wm-more" data-wm-more>显示更多（${fmt(Math.min(limit,visible.length))} / ${fmt(visible.length)}）</button>`:'')+
      (elsewhere.length?`<button class="wm-more" type="button" data-wm-reveal>显示其他类别中的 ${fmt(elsewhere.length)} 处匹配</button>`:'');
  }
  function dismiss() {
    clearCoordinate();selected=null;detail.hidden=true;hideHover();
    const old=list.querySelector('[aria-pressed="true"]');old?.setAttribute('aria-pressed','false');
    camera.refresh();
  }
  function updateFilter(keep=false) {
    keep=keep===true;
    const previous=keep && selected?.id;
    if(!keep){limit=listChunk;list.scrollTop=0;}
    dismiss();
    const enabled=chosen(), query=normalize(search.value);
    visible=[];elsewhere=[];
    for (const p of byMap.get(map.id)) {
      if (query && !p.search.includes(query)) continue;
      if(progressFilter.value==='done' && !isPointDone(p) || progressFilter.value==='pending' && isPointDone(p)) continue;
      if (enabled.has(p.category)||enabled.has(p.alsoCategory)) visible.push(p);
      else if (query) elsewhere.push(p);
    }
    // Separate resource types and bosses so their images remain recognizable
    // in clusters, including when travel points or other layers are also on.
    const layers=new Map();
    for (const point of visible) {
      const layer=point.category==='alpha_pals'||point.category==='custom'||mineralIcons[point.category]?point.category:'locations';
      if (!layers.has(layer)) layers.set(layer,[]);
      layers.get(layer).push(point);
    }
    pointLayers=[...layers.entries()];
    renderList();
    if(previous && visible.some(p=>p.id===previous)) display(pointById.get(previous),false);
    camera.refresh();
  }
  function display(p,center) {
    if (!p || p.map!==map.id) return;
    clearCoordinate();
    const old=list.querySelector('[aria-pressed="true"]');old?.setAttribute('aria-pressed','false');
    selected=p;
    const match=[...list.querySelectorAll('[data-wm-result]')].find(el=>el.dataset.wmResult===p.id);
    match?.setAttribute('aria-pressed','true');
    const coords=coordinateText(p);
    hideHover();
    const extra=[levelLabel(p),p.detail].filter(Boolean).join(' · ');
    const links=[];
    const addRoute=(href,label)=>links.push('<a class="wm-spawn-link" href="'+esc(href)+'">'+esc(label)+' ↗</a>');
    const addLink=(prefix,id,label)=>addRoute(prefix+encodeURIComponent(id),label);
    if(palByKey.has(p.palKey)){
      addLink('#paldex/',p.palKey,'查看'+p.name+'的图鉴');
      addLink('#spawns/',p.palKey,'查看'+p.name+'的刷新位置');
    }
    const mineral=categoryById.get(p.category);
    if(mineral?.itemId)addLink('#items/',mineral.itemId,'查看物品 · '+mineral.name);
    if(mineral?.guide)addRoute(mineral.guide,p.category==='crude_oil'?'查看原油提炼条件':'查看野外采集方式');
    if(mineral?.lootChapter)addLink('#guide/loot/',mineral.lootChapter[0],mineral.lootChapter[1]+'奖励表');
    if(p.rewardId)addLink('#items/',p.rewardId,'查看奖励 · '+p.rewardName);
    if(p.category==='journal')addLink('#guide/journals/',p.id,'阅读这篇手记');
    for(const [id,label] of data.enemyLinks?.[p.id] || []) addLink('#enemies/',id,'敌人图鉴 · '+label);
    for(const [route,label] of data.unitLinks?.[p.id] || []) addRoute(route,'单位图鉴 · '+label);
    for(const [id,label] of data.questLinks?.[p.id] || []) addLink('#guide/quests/',id,'任务 · '+label);
    if(p.nearestId)addLink('#map/point/',p.nearestId,'最近传送点 · '+p.nearest);
    $('wm-detail-body').innerHTML=`<div class="wm-detail-category" style="--wm-color:${color(p.category)}">${pointArt(p)}${esc(catName(p.category))}</div><h3 id="wm-detail-name">${esc(p.name)}</h3>${coords?'<p id="wm-detail-coords">'+esc(map.coordLabel||'坐标')+'　'+esc(coords)+'</p>':''}${extra?'<p class="wm-detail-note">'+esc(extra)+'</p>':''}${links.join(' ')}`;
    $('wm-detail-body').insertAdjacentHTML('beforeend',`<div class="wm-point-actions"><label><input type="checkbox" id="wm-done-check" ${isPointDone(p)?'checked':''}> 已完成</label>${p.category==='custom'?'<button type="button" data-wm-edit>编辑点位</button><button type="button" data-wm-delete>删除</button>':''}</div>`);
    detail.hidden=false;
    if (center) {
      camera.center(p.u,p.v,4);
      if (window.matchMedia('(max-width:760px)').matches) viewport.scrollIntoView({block:'nearest'});
    }
    camera.refresh();
  }
  // Keep the current selection as the originating history entry for a related link.
  // In particular, selecting a different marker after a deep link must replace its old ID.
  document.addEventListener('click',event=>{
    if($('map-view').hidden || !selected || event.defaultPrevented || event.button!==0 || event.metaKey || event.ctrlKey || event.altKey || event.shiftKey)return;
    const href=event.target.closest('a[href^="#"]')?.getAttribute('href');
    if(!href)return;
    const origin='#map/point/'+encodeURIComponent(selected.id);
    history.replaceState({...history.state,atlasSelectedPoint:selected.id},'',origin);
  },true);
  function clusterClick(group) {
    if (!group || group.points.length<2) return;
    const first=group.points[0];
    const distinct=group.points.some(p=>Math.abs(p.u-first.u)+Math.abs(p.v-first.v)>.0001);
    if (distinct && camera.zoom()<15.99) {
      dismiss();
      const u=group.points.reduce((sum,p)=>sum+p.u,0)/group.points.length;
      const v=group.points.reduce((sum,p)=>sum+p.v,0)/group.points.length;
      camera.center(u,v,camera.zoom()*2);
      return;
    }
    dismiss();
    $('wm-detail-body').innerHTML=`<h3 id="wm-detail-name">附近的点位</h3><p class="wm-cluster-notice">共 ${fmt(group.points.length)} 处，选择一个查看详情。</p><div class="wm-cluster-list">${group.points.map(p=>`<button type="button" data-wm-pick="${esc(p.id)}" style="--wm-color:${color(p.category)}">${pointArt(p)}${esc(p.name)}　${esc(levelLabel(p))}</button>`).join('')}</div>`;
    detail.hidden=false;
  }
  const hover=$('wm-hover');
  function hideHover() {if(hovered){hovered=null;hover.hidden=true;}}
  function positionHover() {
    if (!hovered || !hovered.isConnected) {hideHover();return;}
    const area=viewport.getBoundingClientRect(), box=hovered.getBoundingClientRect();
    if (box.bottom<=area.top || box.top>=area.bottom || box.right<=area.left || box.left>=area.right) {hideHover();return;}
    const left=(box.left+box.right-hover.offsetWidth)/2-area.left;
    const above=box.top-area.top-hover.offsetHeight-6;
    hover.style.left=Math.max(3,Math.min(viewport.clientWidth-hover.offsetWidth-3,left))+'px';
    hover.style.top=(above>=3?above:Math.min(viewport.clientHeight-hover.offsetHeight-3,box.bottom-area.top+6))+'px';
  }
  function showHover(pin) {
    if (!pin || viewport.classList.contains('wm-panning'))return;
    const p=pointById.get(pin.dataset.wmPoint);
    if (!p) {hideHover();return;}
    hovered=pin;
    hover.innerHTML='<strong>'+esc(p.name)+'</strong>'+
      (levelLabel(p)?'<span>'+esc(levelLabel(p))+'</span>':'');
    hover.hidden=false;positionHover();
  }
  function draw(g) {
    if (!map) return;
    // With a sparse layer the clipped DOM can keep every pin. No marker work
    // is then needed during a drag, even when the camera crosses the map.
    g.panComplete=visible.length<=160;
    if(coordinateTarget){
      coordinateMark.style.left=coordinateTarget.u*g.w+'px';
      coordinateMark.style.top=coordinateTarget.v*g.h+'px';
    }
    const scale=camera.zoom(), retained=new Set();
    const fragment=document.createDocumentFragment();
    let firstVisible=null;
    // Several resource layers can occupy the same area. Space their clusters
    // farther apart in dense views, tapering back to the normal hit radius
    // as the user zooms in on a site or narrows the category filters.
    const inView=visible.length*Math.min(1,g.width/g.w)*Math.min(1,g.height/g.h);
    const density=Math.min(2.2,Math.max(1,Math.sqrt(inView*1500/(g.width*g.height))));
    outputGroups=new Map();
    for (const [layer,points] of pointLayers) {
      const bosses=layer==='alpha_pals', minerals=Boolean(mineralIcons[layer]);
      const keepBosses=bosses && (scale>=2 || pointLayers.length===1);
      const radius=bosses?17:(scale<2?32:26)*density;
      for (const group of clusterMapPoints(points,g,radius,selected,keepBosses)) {
        const p=group.points[0], n=group.points.length, multi=n>1;
        const art=Boolean(group.category);
        const key=multi?'c'+p.id+'-'+(group.category||'mixed'):'p'+p.id;
        retained.add(key);
        if(multi)outputGroups.set(key,group);
        if(!firstVisible && group.x+g.x>=0 && group.y+g.y>=0 && group.x+g.x<=g.width && group.y+g.y<=g.height) firstVisible=key;
        let pin=livePins.get(key);
        if(pin && g.panOnly)continue;
        if (!pin) {
          pin=document.createElement('button');pin.type='button';pin.tabIndex=-1;
          pin.dataset.wmKey=key;
          pin.dataset.category=p.category;
          if (bosses) pin.classList.add('wm-pal-pin');
          if (minerals) pin.classList.add('wm-mineral-pin');
          if (art && !bosses && !minerals) pin.classList.add('wm-game-pin');
          if (multi) {
            pin.dataset.wmCluster=key;pin.classList.add('wm-cluster');
            if (art) {
              const symbol=bosses || minerals ? pointArt(p) : icon(group.category);
              pin.innerHTML=symbol+'<span class="wm-pin-total"></span>';
            }
          } else {
            pin.dataset.wmPoint=p.id; pin.innerHTML=pointArt(p);
          }
          livePins.set(key,pin);fragment.append(pin);
        }
        if (multi) {
          const counter=art?pin.querySelector('.wm-pin-total'):pin;
          const done=doneIds.size?group.points.reduce((total,item)=>total+Number(isPointDone(item)),0):0;
          pin.classList.toggle('wm-done',done===n);
          if (pin.dataset.count!==String(n)||pin.dataset.completed!==String(done)) {
            pin.dataset.completed=String(done);
            pin.dataset.count=String(n);counter.textContent=String(n);
            const text=fmt(n)+' 处'+(group.category?catName(group.category):'点位')+(done?'，已完成 '+fmt(done)+' 处':'')+'，点击展开';
            pin.title=text;pin.setAttribute('aria-label',text);
          }
        } else {
          pin.classList.toggle('wm-selected',p===selected);
          const done=isPointDone(p);
          pin.classList.toggle('wm-done',done);
          const label=[p.name,catName(p.category),levelLabel(p),done?'已完成':''].filter(Boolean).join(' · ');
          if(pin.getAttribute('aria-label')!==label)pin.setAttribute('aria-label',label);
        }
        const tone=multi && !group.category ? '#304f4c' : color(group.category||p.category);
        if(pin.style.getPropertyValue('--wm-color')!==tone)pin.style.setProperty('--wm-color',tone);
        pin.style.left=group.x+'px';pin.style.top=group.y+'px';
      }
    }
    for (const [key,pin] of livePins) {
      if (!retained.has(key)) {pin.remove();livePins.delete(key);}
    }
    markerLayer.classList.toggle('wm-dense',retained.size>125);
    if(fragment.childElementCount)markerLayer.append(fragment);
    if (!retained.has(tabKey)) tabKey=firstVisible||retained.values().next().value;
    const active=livePins.get(tabKey);
    if (active && active.tabIndex!==0) active.tabIndex=0;
    if (hovered) positionHover();
  }
  markerLayer.addEventListener('pointerover',e=>{
    if(e.pointerType!=='touch') showHover(e.target.closest('button'));
  });
  markerLayer.addEventListener('pointerout',e=>{
    if(!e.target.closest('button')?.contains(e.relatedTarget)) hideHover();
  });
  markerLayer.addEventListener('pointerdown',hideHover);
  markerLayer.addEventListener('focusin',e=>{
    const pin=e.target.closest('button');
    if (!pin) return;
    const old=livePins.get(tabKey);
    if (old && old!==pin) old.tabIndex=-1;
    tabKey=pin.dataset.wmKey;pin.tabIndex=0;showHover(pin);
  });
  markerLayer.addEventListener('focusout',hideHover);
  markerLayer.addEventListener('keydown',e=>{
    const pin=e.target.closest('button');
    if (!pin) return;
    if (e.key==='Escape') {hideHover();dismiss();viewport.focus({preventScroll:true});return;}
    if (!['ArrowLeft','ArrowRight','ArrowUp','ArrowDown','Home','End'].includes(e.key)) return;
    e.preventDefault();
    const pins=camera.visibleMarkers();
    if(!pins.length)return;
    const forward=e.key==='ArrowRight'||e.key==='ArrowDown';
    const index=e.key==='Home'?0:e.key==='End'?pins.length-1:(pins.indexOf(pin)+(forward?1:-1)+pins.length)%pins.length;
    pins[index]?.focus({preventScroll:true});
  });
  function renderSources() {
    const entries=data.sources || data.meta?.sources || [];
    const links=entries.filter(v=>typeof v.url==='string' && /^https?:\/\//.test(v.url));
    sources.innerHTML=(map.note?esc(map.note)+'　':'')+(links.length?'地图资料　'+links.map(v=>`<a href="${esc(v.url)}" target="_blank" rel="noopener noreferrer">${esc(v.name||v.label||v.url)}</a>`).join(' · '):'');
  }
  function updateSummary() {
    $('wm-summary').textContent=map.name+' · '+fmt(byMap.get(map.id).length)+' 处点位';
  }
  function reportNote(text) {
    const out=$('wm-note-message');out.hidden=!text;out.textContent=text;
  }
  function setPicking(on) {
    $('wm-place').hidden=!on;
    viewport.classList.toggle('wm-placing',on);
  }
  function cancelDraft() {
    draft=null;setPicking(false);
    if(pinDialog.open) pinDialog.close();
  }
  function openEditor(position) {
    if(!draft) return;
    if(position) draft.position=position;
    const pos=draft.position;
    $('wm-pin-x').value=pos?pos.x:'';
    $('wm-pin-y').value=pos?pos.y:'';
    $('wm-pin-name').value=draft.name;$('wm-pin-note').value=draft.note;
    $('wm-pin-title').textContent=draft.edit?'编辑点位':'添加点位';
    $('wm-pin-map-label').textContent=map.name+' · '+map.coordLabel;
    $('wm-pin-error').hidden=true;setPicking(false);
    if(!pinDialog.open) pinDialog.showModal();
    $('wm-pin-name').focus();
  }
  function startPin() {
    cancelDraft();reportNote('');dismiss();
    draft={worldId:notesWorld,region:map.id,id:worlds.newMapPinId(),name:'',note:'',position:null,edit:false};
    setPicking(true);viewport.focus({preventScroll:true});
    if(window.matchMedia('(max-width:760px)').matches) viewport.scrollIntoView({block:'nearest'});
  }
  function pickLocation(e) {
    if(!draft || $('wm-place').hidden) return false;
    if(e.type==='keydown') {cancelDraft();return true;}
    const g=camera.geometry(),box=viewport.getBoundingClientRect();
    if(!g) return true;
    const u=(e.clientX-box.left-g.x)/g.w,v=(e.clientY-box.top-g.y)/g.h;
    if(u<0||u>1||v<0||v>1){reportNote('请选择底图范围内的位置。');return true;}
    reportNote('');openEditor({u,v,...coordinatesAt(map.id,u,v)});return true;
  }
  $('wm-new-pin').addEventListener('click',startPin);
  $('wm-stop-place').addEventListener('click',()=>{cancelDraft();$('wm-new-pin').focus();});
  $('wm-type-position').addEventListener('click',()=>openEditor());
  $('wm-pin-cancel').addEventListener('click',cancelDraft);
  pinDialog.addEventListener('cancel',()=>{draft=null;setPicking(false);});
  $('wm-pick-again').addEventListener('click',()=>{
    if(!draft) return;
    draft.name=$('wm-pin-name').value;draft.note=$('wm-pin-note').value;
    pinDialog.close();setPicking(true);viewport.focus({preventScroll:true});
  });
  $('wm-pin-form').addEventListener('submit',async e=>{
    e.preventDefault();
    const edit=draft;
    if(!edit || $('wm-pin-save').disabled) return;
    const error=$('wm-pin-error');
    try {
      const x=$('wm-pin-x').valueAsNumber,y=$('wm-pin-y').valueAsNumber,b=bounds[edit.region];
      const pos=edit.position;
      // Keep the exact clicked position when the rounded game coordinates were not edited.
      const u=pos&&pos.x===x?pos.u:(x-b.left)/b.span,v=pos&&pos.y===y?pos.v:(b.top-y)/b.span;
      if(![u,v].every(n=>Number.isFinite(n)&&n>=0&&n<=1))
        throw Error('坐标超出'+map.name+'底图范围，可点「在地图上选位置」。');
      const fields={map:edit.region,u,v,name:$('wm-pin-name').value,note:$('wm-pin-note').value};
      $('wm-pin-save').disabled=true;
      await worlds.updateMapNote(edit.worldId,{id:edit.id,type:edit.edit?'edit':'add',fields});
      if(draft!==edit)return;
      cancelDraft();focusPoint(edit.id);reportNote('已保存「'+fields.name.trim()+'」。');
    } catch(err){if(draft===edit){error.hidden=false;error.textContent=err.message;}}
    finally{$('wm-pin-save').disabled=false;}
  });
  progressFilter.addEventListener('change',()=>updateFilter());
  async function setChecked(check,id) {
    const focused=document.activeElement===check;
    check.disabled=true;
    try{
      await worlds.updateMapNote(notesWorld,{id,type:'done',done:check.checked});
      if(focused && !document.activeElement?.closest('dialog[open]')) {
        const next=check.id==='wm-done-check'?(!detail.hidden&&$('wm-done-check')):list.querySelector('[data-wm-check="'+CSS.escape(id)+'"]');
        (next||progressFilter).focus({preventScroll:true});
      }
    }
    catch(err){check.checked=!check.checked;check.disabled=false;reportNote(err.message);}
  }
  detail.addEventListener('change',e=>{
    if(e.target.id==='wm-done-check'&&selected) setChecked(e.target,selected.id);
  });
  list.addEventListener('change',e=>{
    if(e.target.dataset.wmCheck) setChecked(e.target,e.target.dataset.wmCheck);
  });
  detail.addEventListener('click',async e=>{
    if(selected?.category!=='custom')return;
    if(e.target.closest('[data-wm-edit]')) {
      draft={id:selected.id,region:map.id,worldId:notesWorld,name:selected.name,note:selected.detail||'',edit:true,
        position:{u:selected.u,v:selected.v,x:selected.x,y:selected.y}};
      openEditor();
    } else if(e.target.closest('[data-wm-delete]')) {
      const actions=e.target.closest('.wm-point-actions');
      actions.innerHTML='<span>删除这个点位？</span><button type="button" data-wm-confirm-delete>确认删除</button><button type="button" data-wm-keep>取消</button>';
      actions.querySelector('[data-wm-keep]').focus();
    } else if(e.target.closest('[data-wm-keep]')) display(selected,false);
    else if(e.target.closest('[data-wm-confirm-delete]')) {
      try{await worlds.updateMapNote(notesWorld,{id:selected.id,type:'delete'});reportNote('已删除自定义点位。');}
      catch(err){reportNote(err.message);}
    }
  });
  function switchMap(id) {
    const next=mapById.get(id);if (!next) return;
    cancelDraft();
    map=next;mapSelect.value=next.id;viewport.dataset.map=id;search.value='';
    $('wm-category-expand').setAttribute('aria-expanded','false');categoriesBox.closest('.wm-sidebar').classList.remove('wm-expanded');
    if (!chosenByMap.has(id)) {
      const counts=categoryCounts.get(id);
      const enabled=categories.filter(c=>counts.has(c.id)&&c.default).map(c=>c.id);
      chosenByMap.set(id,new Set(enabled.length?enabled:[...counts.keys()].slice(0,2)));
    }
    updateSummary();
    sheet.alt=next.name+'地图';sheet.src=images[id];

    camera.deactivate();renderCategories();renderSources();camera.fit();updateFilter();
  }
  sheet.addEventListener('load',()=>camera.refresh());
  function focusCoordinates(region,x,y) {
    const b=bounds[region];
    if(!b || !Number.isFinite(x) || !Number.isFinite(y))return false;
    const u=(x-b.left)/b.span,v=(b.top-y)/b.span;
    if(u<0||u>1||v<0||v>1)return false;
    if(map.id!==region)switchMap(region);
    dismiss();coordinateTarget={u,v};coordinateMark.hidden=false;
    $('wm-detail-body').innerHTML='<h3 id="wm-detail-name">坐标定位</h3><p id="wm-detail-coords">'+esc(map.coordLabel)+'　'+esc(x)+'，'+esc(y)+'</p>';
    detail.hidden=false;camera.center(u,v,5);camera.refresh();
    if(window.matchMedia('(max-width:760px)').matches)viewport.scrollIntoView({block:'nearest'});
    return true;
  }
  function focusPoint(id) {
    const point=pointById.get(id);
    if (!point) return false;
    // When a detail page returns to the same live marker, the map still holds
    // the original search, visible categories and camera position.
    if(history.state?.atlasSelectedPoint===id && selected?.id===id && !detail.hidden)return true;
    if (point.map!==map.id) switchMap(point.map);
    search.value='';
    if(progressFilter.value==='done'&&!isPointDone(point)||progressFilter.value==='pending'&&isPointDone(point))progressFilter.value='all';
    chosen().add(point.category);
    renderCategories();updateFilter();display(point,true);
    return true;
  }
  mapSelect.addEventListener('change',()=>switchMap(mapSelect.value));
  function focusCategory(category,region='palpagos',query='') {
    if(!mapById.has(region)||!categoryById.has(category))return false;
    if(map.id!==region)switchMap(region);
    search.value=query;progressFilter.value='all';
    chosenByMap.set(region,new Set([category]));
    renderCategories();updateFilter();camera.fit();return true;
  }
  $('wm-category-expand').addEventListener('click',()=>{
    const button=$('wm-category-expand'),expand=button.getAttribute('aria-expanded')!=='true';
    button.setAttribute('aria-expanded',String(expand));categoriesBox.closest('.wm-sidebar').classList.toggle('wm-expanded',expand);
    if(!expand) categoriesBox.scrollTop=0;
    updateExpandControl();
  });
  categoriesBox.addEventListener('change',e=>{
    const input=e.target.closest('[data-wm-category]');if (!input) return;
    if (input.checked) chosen().add(input.dataset.wmCategory);else chosen().delete(input.dataset.wmCategory);
    updateFilter();
  });
  $('wm-all').addEventListener('click',()=>{chosenByMap.set(map.id,new Set(categoryCounts.get(map.id).keys()));renderCategories();updateFilter();});
  $('wm-none').addEventListener('click',()=>{chosen().clear();renderCategories();updateFilter();});
  search.addEventListener('input',e=>{if(!e.isComposing) updateFilter();});
  search.addEventListener('compositionend',updateFilter);
  list.addEventListener('click',e=>{
    const item=e.target.closest('[data-wm-result]');
    if (item) {display(pointById.get(item.dataset.wmResult),true);return;}
    if (e.target.closest('[data-wm-more]')) {
      const next=visible[limit];limit+=listChunk;renderList();
      const target=[...list.querySelectorAll('[data-wm-result]')].find(el=>el.dataset.wmResult===next?.id);
      target?.focus({preventScroll:true});return;
    }
    if (e.target.closest('[data-wm-reveal]')) {for(const p of elsewhere) chosen().add(p.category);renderCategories();updateFilter();}
  });
  detail.addEventListener('click',e=>{const el=e.target.closest('[data-wm-pick]');if(el)display(pointById.get(el.dataset.wmPick),true);});
  $('wm-detail-close').addEventListener('click',()=>{dismiss();viewport.focus({preventScroll:true});});
  const camera=createMapCamera({
    viewport,sheet,markerLayer,onMove:hideHover,zoomLabel:$('wm-zoom-level'),zoomIn:$('wm-zoom-in'),zoomOut:$('wm-zoom-out'),fitButton:$('wm-fit'),
    uiSelector:'[data-wm-ui]',markerSelector:'[data-wm-point],[data-wm-cluster]',onDraw:draw,onBlank:e=>{if(!pickLocation(e))dismiss();},
    onPoint(pin) {
      if (pin.dataset.wmPoint) display(pointById.get(pin.dataset.wmPoint),false);
      else clusterClick(outputGroups.get(pin.dataset.wmCluster));
    }
  });
  new ResizeObserver(()=>{if(map)updateExpandControl();}).observe(viewport);
  worlds.events.addEventListener('storage',event=>{
    if(event.key===null || event.key?.startsWith('pal-atlas.map.v1.')) mapNotesChanged(event);
  });
  mapNotesChanged();switchMap(data.maps[0].id);
  return {focusPoint,focusCategory,focusCoordinates,refresh:()=>{if(notesDirty)mapNotesChanged();camera.refresh();},deactivate:()=>{cancelDraft();hideHover();camera.deactivate();}};
}

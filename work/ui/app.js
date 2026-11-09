(() => {
  'use strict';
  const db = JSON.parse(document.getElementById('db').textContent);
  const portraits = JSON.parse(document.getElementById('portraits').textContent);
  const {species, meta} = db;
  const recipes=createRecipeIndex(db);
  const byId = id => document.getElementById(id);
  const appName=document.querySelector('meta[name="application-name"]').content;
  function setPageTitle(page){document.title=page+' · '+appName;}
  const escape = text => String(text).replace(/[&<>"']/g, c => ({'&':'&amp;', '<':'&lt;', '>':'&gt;', '"':'&quot;', "'":'&#39;'}[c]));
  const format = number => number.toLocaleString('en-US');
  const shapes = {
    enemy:'<path d="m4 3 4.5 1.5 11 13-2 2-12-12L4 3Zm11 12 4.5-3.5M19 20l2 2M13.5 8l3-3.5L21 3l-1.5 4.5-3.5 3M8 15l-4.5 5 1.5 1.5 5-5M4 12l6 6"/>',
    compass:'<circle cx="12" cy="12" r="9"/><path d="m15.8 8.2-2.4 5.2-5.2 2.4 2.4-5.2Z"/>',
    layers:'<path d="m3 8 9-5 9 5-9 5-9-5Zm0 5 9 5 9-5m-18 5 9 5 9-5"/>',
    paldex:'<rect x="3" y="3" width="18" height="18" rx="3"/><path d="M6.5 3v18m3.7-6.5c0-1.5 1.5-3.1 2.9-3.1s2.9 1.6 2.9 3.1c0 .9-.6 1.5-1.5 1.4a5 5 0 0 0-2.8 0c-.9.1-1.5-.5-1.5-1.4Z"/><circle cx="10.1" cy="9.6" r=".6"/><circle cx="13" cy="8.3" r=".6"/><circle cx="16" cy="9.5" r=".6"/>',
    relic:'<path d="m13 3 6.5 5-3.2 10-8.8 2-3-8.2L13 3Zm0 0-1.8 9m-6.7-.2 6.7.2 5.1 6M11.2 12l8.3-4"/>',
    ranch:'<path d="M3 10.5 12 4l9 6.5M5 9.2V20h14V9.2M9 20v-6.8h6V20m-6-6.8 6 6.8m0-6.8L9 20"/><path d="M10.5 9h3"/>',
    items:'<path d="m3 6.5 9-4 9 4v11l-9 4-9-4Zm0 0 9 4 9-4m-9 4v11m-5-17 10 4.3v4"/>',
    worldmap:'<path d="m2.5 5.7 6-2.4 7 2.7 6-2.4v15l-6 2.4-7-2.7-6 2.4Zm6-2.4v15m7-12.3v15"/>',
    pin:'<path d="M19.5 10c0 5.8-7.5 11.3-7.5 11.3S4.5 15.8 4.5 10a7.5 7.5 0 0 1 15 0Z"/><circle cx="12" cy="10" r="2.6"/>',
    menu:'<path d="M3.5 6.5h17m-17 5.5h17m-17 5.5h17"/>',
    bell:'<path d="M18 8a6 6 0 0 0-12 0c0 7-3 7.5-3 9h18c0-1.5-3-2-3-9ZM9.5 20a3 3 0 0 0 5 0"/>',
    info:'<circle cx="12" cy="12" r="9"/><path d="M12 11v6m0-10v.2"/>',
    settings:'<path d="m9.5 3 .6-1h3.8l.6 2.2 1.7 1 2.2-.6 1.9 3.3-1.6 1.6v2l1.6 1.6-1.9 3.3-2.2-.6-1.7 1-.6 2.2h-3.8l-.6-2.2-1.7-1-2.2.6-1.9-3.3 1.6-1.6v-2L3.7 7.9l1.9-3.3 2.2.6 1.7-1Z" transform="translate(0 1.5)"/><circle cx="12" cy="12" r="3"/>',
    book:'<path d="M3 4.5c3.8-1 6-.4 9 1.8 3-2.2 5.2-2.8 9-1.8v14.2c-4-.9-6.2-.2-9 1.8-2.8-2-5-2.7-9-1.8Z"/><path d="M12 6.3v14"/>',
    warehouse:'<path d="m2.5 9.5 9.5-6 9.5 6M4.5 8.3V21h15V8.3"/><path d="M8.2 21V12h7.6v9M8.2 16h7.6"/>',
    target:'<circle cx="11" cy="13" r="8"/><circle cx="11" cy="13" r="4"/><path d="m11 13 8-8m-1-3v4h4"/>',
    list:'<rect x="5" y="3" width="15" height="18" rx="2"/><path d="M9 3v18m4-13h4m-4 4h4M3 7h3m-3 5h3m-3 5h3"/>',
    egg:'<path d="M20 15.1c0 4.5-3.6 7-8 7s-8-2.5-8-7C4 9.6 8.3 2 12 2s8 7.6 8 13.1Z"/><path d="m6 14 3.5 1.8 3-4 2.5 2 3-2"/>',
    branch:'<circle cx="6" cy="5.5" r="2.5"/><circle cx="6" cy="18.5" r="2.5"/><circle cx="18.5" cy="6.5" r="2.5"/><path d="M6 8v8m0-1c2-5 12.5-.3 12.5-6"/>',
    parents:'<rect x="9" y="3" width="6" height="5" rx="1.4"/><rect x="2" y="16" width="6" height="5" rx="1.4"/><rect x="16" y="16" width="6" height="5" rx="1.4"/><path d="M12 8v4m-7 4v-4h14v4"/>',
    search:'<circle cx="10.5" cy="10.5" r="6.5"/><path d="m15.5 15.5 4.8 4.8"/>',
    check:'<path d="m5 12 4.5 4.5L19 7"/>',
    right:'<path d="m9.3 5 7 7-7 7"/>', left:'<path d="m14.7 5-7 7 7 7"/>',
    last:'<path d="m6 5 7 7-7 7m12-14v14"/>', first:'<path d="m18 5-7 7 7 7M6 5v14"/>',
    arrow:'<path d="M4 12h15m-5-5 5 5-5 5"/>', plus:'<path d="M5 12h14m-7-7v14"/>',
    down:'<path d="m5 9 7 7 7-7"/>', close:'<path d="m6 6 12 12M18 6 6 18"/>',
    neutral:'<path d="m12 2.5 2.3 6.6 6.7 2.7-6.7 2.6-2.3 6.8-2.4-6.8-6.6-2.6 6.6-2.7Z"/>',
    grass:'<path d="M20.5 3.5C9.5 3.8 5 7 5 13a6 6 0 0 0 6 6c5.2 0 9.4-6.8 9.5-15.5ZM4 21 16 9"/>',
    fire:'<path d="M13 2c1.3 5.4 5 6.2 4.5 9.7 1.4-.6 2.3-1.8 2.5-2.8 3 8-1.6 12-7.5 12-5.4 0-9.2-3.9-8-9.1.5-2.1 1.9-4.3 3.3-6 .3 2.4.8 3.2 1.5 3.7C11.4 8 12.8 5.7 13 2Z"/>',
    water:'<path d="M12 2C10 6 5 10.4 5 15a7 7 0 0 0 14 0C19 10.4 14 6 12 2Z"/><path d="M9.5 17c.7.8 1.4 1.2 2.6 1.2"/>',
    electric:'<path d="m14 2-11 12h7.5L9.5 22 21 9h-8Z"/>',
    dark:'<path d="M20.5 15.5A9 9 0 0 1 8.7 3a9.3 9.3 0 1 0 11.8 12.5Z"/>',
    ice:'<path d="M12 2v20M3.4 7l17.2 10m0-10L3.4 17M9.5 4.5 12 7l2.5-2.5m-5 15L12 17l2.5 2.5"/>',
    ground:'<path d="m2 20 7-12 4 6 2-3 7 9Zm5-8 2 2 2-2"/>',
    dragon:'<path d="m5 21 2-6-3-1 3-4 7-5V2l4 3 3 1-2 5-4 2-3 6-7 2Z"/><path d="m14 9 2-.5"/>'
  };
  const svg = name => `<svg viewBox="0 0 24 24" aria-hidden="true">${shapes[name] || ''}</svg>`;
  const icon = name => `<span class="inline-icon" aria-hidden="true">${svg(name)}</span>`;
  function fillIcons(root){root.querySelectorAll('[data-icon]').forEach(el=>{el.innerHTML=svg(el.dataset.icon);});}
  fillIcons(document);
  document.addEventListener('atlas-panel-mounted',event=>fillIcons(event.target));
  const types = {
    neutral:['无','#eeecdc','#6d6041','#dcd8be'], fire:['火','#fff0e5','#9d4928','#ecd2bf'],
    water:['水','#e5f2f7','#386b86','#c8e1ea'], grass:['草','#e7f2dd','#4f6931','#cfe1be'],
    electric:['雷','#faf1ca','#806421','#e9dcab'], ice:['冰','#e5f4f3','#32716e','#c9e5e4'],
    ground:['地','#f5e9d7','#81603b','#e7d7bf'], dark:['暗','#eee8f4','#715989','#dacfe5'],
    dragon:['龙','#e9e8f6','#635489','#d8d5e9']
  };
  function typeStyle(type) {
    const [,bg,fg,border] = types[type] || types.neutral;
    return `--tint:${bg};--tone:${fg};--edge:${border}`;
  }
  const badge = type => `<span class="type-badge" style="${typeStyle(type)}" aria-label="${types[type][0]}属性">${icon(type)}${types[type][0]}</span>`;
  const typeSet = pal => pal.types.map(badge).join('');
  const dex = pal => pal.dex ? '#' + pal.dex : pal.group || '';
  const spawnHref = index => '#spawns/'+encodeURIComponent(species[index].key);
  function spawnLink(index,label=species[index].zh) {
    const name=escape(label);
    return `<a class="habitat-name" href="${spawnHref(index)}" title="查看${name}的刷新位置">${name}</a>`;
  }
  const avatar = (pal,loading='lazy') => `<span class="avatar" aria-hidden="true" style="${typeStyle(pal.types[0])}">${portraits[pal.en] ? `<img alt="" src="${portraits[pal.en]}" loading="${loading}" decoding="async" draggable="false">` : icon('egg')}</span>`;
  // Keep list markup short on the portable page, too. Request images as
  // their rows approach the window or scroll into the species selector.
  const listAvatars=species.map((pal,index)=>`<span class="avatar" aria-hidden="true" style="${typeStyle(pal.types[0])}">${portraits[pal.en]?`<img alt="" data-portrait="${index}" loading="lazy" decoding="async" draggable="false">`:icon('egg')}</span>`);
  const portraitObservers=new WeakMap();
  const previewBlock=byId('lookup-thumbnails');
  const lookupArt=previewBlock?JSON.parse(previewBlock.textContent):null;
  function loadPortrait(img,small=false) {
    const key=species[Number(img.dataset.portrait)].en;
    img.src=(small && lookupArt?.[key]) || portraits[key];
    img.removeAttribute('data-portrait');
  }
  function fillPortraits(container) {
    const pending=container.querySelectorAll('img[data-portrait]');
    // The Paldeck builds reusable cards in a detached template. Give these
    // native lazy sources so scroll-to-card and immediate decode work together.
    if(container.nodeType===Node.DOCUMENT_FRAGMENT_NODE || !window.IntersectionObserver) {
      pending.forEach(img=>loadPortrait(img,container.id==='pair-rows'));return;
    }
    let observer=portraitObservers.get(container);
    if(observer) observer.disconnect();
    if(!pending.length) return;
    if(!observer) {
      observer=new IntersectionObserver(entries=>{
        for(const entry of entries) if(entry.isIntersecting) {
          observer.unobserve(entry.target);
          if(entry.target.hasAttribute('data-portrait')) loadPortrait(entry.target,container.id==='pair-rows');
        }
      },{rootMargin:'160px',scrollMargin:'120px'});
      portraitObservers.set(container,observer);
    }
    pending.forEach(img=>observer.observe(img));
  }
  const normalize = value => String(value).normalize('NFKC').trim().toLowerCase().replace(/[\s·’'._（）()-]+/g, '');
  const searchIndex = species.map(pal => [pal.zh,pal.en,...(pal.aliases || [])].map(normalize));
  function matchScore(index, query) {
    if (!query) return 1;
    const number = query.match(/^#?0*(\d+)([a-z]?)$/);
    if (number) {
      const code = species[index].dex.replace(/^0+/, '').toLowerCase();
      const wanted = Number(number[1]) + number[2];
      if (code === wanted) return 4;
      return code.startsWith(wanted) ? 1 : 0;
    }
    const terms = searchIndex[index];
    if (terms.includes(query)) return 4;
    if (terms.some(term => term.startsWith(query))) return 3;
    return terms.some(term => term.includes(query)) ? 2 : 0;
  }
  const pageSizeKey = 'pal-atlas.result-page-size.v1';
  const pageSizeChoices = ['30','60','100','all'];
  const pageSizeControls = [byId('lookup-page-size'),byId('wh-page-size')];
  let pageSizeValue = '30';
  try {
    const saved = localStorage.getItem(pageSizeKey);
    if (pageSizeChoices.includes(saved)) pageSizeValue = saved;
  } catch { /* The page size still applies for this session. */ }
  for (const select of pageSizeControls) {
    for (const value of pageSizeChoices) select.add(new Option(value === 'all' ? '全部' : value + ' 条',value));
    select.value = pageSizeValue;
  }
  function resultsPerPage(total) {
    return pageSizeValue === 'all' ? Math.max(1,total) : Number(pageSizeValue);
  }
  const allSpecies = species.map((_, index) => index);
  const state = {mode:'forward', selected:0, visible:allSpecies, page:1, rows:[]};
  const who = value => value === 'F' ? '雌性' : '雄性';
  byId('species-count').textContent = format(species.length);
  byId('recipe-count').textContent = format(recipes.count);
  for (const [value,[label]] of Object.entries(types)) byId('element').add(new Option(label + '属性', value));
  const catalog=byId('species-list');
  const checkmark=document.createElement('span');
  checkmark.className='selected-check';checkmark.innerHTML=icon('check');
  let catalogRows, catalogVisible, catalogSelected;
  const catalogObserver=window.IntersectionObserver?new IntersectionObserver(entries=>{
    for(const entry of entries) if(entry.isIntersecting) fillCatalogRow(entry.target);
  },{root:catalog,rootMargin:'220px 0px'}):null;
  function fillCatalogRow(button) {
    if(!button || button.hasAttribute('data-rendered')) return;
    const index=Number(button.dataset.species),pal=species[index];
    button.innerHTML=listAvatars[index]+'<span class="species-info"><strong>'+escape(pal.zh)+'</strong><span class="dex">'+escape(dex(pal))+'</span></span>';
    const img=button.querySelector('[data-portrait]');
    if(img) loadPortrait(img,true);
    button.setAttribute('data-rendered','');
    catalogObserver?.unobserve(button);
  }
  function renderCatalog() {
    const {visible,selected}=state;
    if(!catalogRows) {
      // Every row immediately supports keyboard, find-in-page and scrollbar
      // navigation. Build its portrait and secondary markup as it approaches view.
      catalog.innerHTML=species.map((pal,index)=>`<button type="button" data-species="${index}" class="species-row" aria-pressed="false" tabindex="-1">${escape(pal.zh)} ${escape(dex(pal))}</button>`).join('');
      catalogRows=[...catalog.children];
      catalogVisible=allSpecies;
      if(catalogObserver)catalogRows.forEach(button=>catalogObserver.observe(button));
      else catalogRows.forEach(fillCatalogRow);
      visible.slice(0,12).forEach(index=>fillCatalogRow(catalogRows[index]));
    }
    if(catalogVisible!==visible) {
      if(visible.length) catalog.replaceChildren(...visible.map(index=>catalogRows[index]));
      else catalog.innerHTML='<div class="no-results"><strong>没有找到这只帕鲁</strong>试试其他名称或图鉴编号。</div>';
      catalogVisible=visible;
      visible.slice(0,12).forEach(index=>fillCatalogRow(catalogRows[index]));
    }
    if(catalogSelected!==selected) {
      if(catalogSelected!=null) {
        const previous=catalogRows[catalogSelected];
        previous.classList.remove('selected');previous.setAttribute('aria-pressed','false');previous.tabIndex=-1;
      }
      if(selected!=null) {
        const next=catalogRows[selected];
        fillCatalogRow(next);
        next.classList.add('selected');next.setAttribute('aria-pressed','true');next.tabIndex=0;
        next.append(checkmark);
      } else checkmark.remove();
      catalogSelected=selected;
    }
    byId('catalog-count').textContent=visible.length===species.length ? `共 ${format(species.length)} 个图鉴条目` : `找到 ${format(visible.length)} 个条目`;
  }
  function updateSelection() {
    const pal = species[state.selected];
    const forward = state.mode === 'forward';
    byId('selection-label').textContent = forward ? '当前亲代' : '目标子代';
    byId('specimen').innerHTML = pal ? avatar(pal,'eager') : '<span class="avatar">'+icon('egg')+'</span>';
    byId('selection-name').textContent = pal ? pal.zh : '请选择帕鲁';
    byId('selected-types').innerHTML = pal ? typeSet(pal) : '';
    byId('selection-id').textContent = pal ? `${dex(pal)}　·　${pal.en}` : '';
    const habitat=byId('lookup-spawns');
    habitat.hidden=!pal;
    const profile=byId('lookup-paldex');
    profile.hidden=!pal;
    if (pal) {
      profile.href='#paldex/'+encodeURIComponent(pal.key);
      habitat.href=spawnHref(state.selected);
      habitat.setAttribute('aria-label','查看'+pal.zh+'的刷新位置');
    }
    byId('pair-key').innerHTML = forward ? '配种伙伴 '+icon('arrow')+' 孵化子代' : '亲代 A '+icon('plus')+' 亲代 B <span>一雌一雄</span>';
    byId('result-title').textContent = forward ? '配种组合' : '双亲组合';
    byId('filter').placeholder = forward ? '筛选伙伴或子代' : '筛选任一亲代';
    byId('element').setAttribute('aria-label',forward ? '筛选子代属性' : '筛选任一亲代属性');
    byId('element').title = forward ? '按子代属性筛选' : '按任一亲代属性筛选';
    updateStockButton();
  }
  function resetFilter() {
    byId('filter').value = '';
    byId('element').value = '';
    state.page = 1;
  }
  function select(index,syncRoute=true) {
    if (index !== state.selected) resetFilter();
    state.selected = index;
    renderCatalog();
    updateSelection();
    runQuery();
    if (syncRoute && /^#lookup\//.test(location.hash) && species[index]) {
      const href='#lookup/'+encodeURIComponent(species[index].key);
      if (location.hash!==href) history.replaceState(history.state,'',href);
      byId('section-breeding').href=href;
    }
  }
  const partner = row => row[0] === state.selected ? row[1] : row[0];
  let querySpecies,queryMode,orderedRows=[];
  function runQuery() {
    const q = normalize(byId('filter').value);
    const type = byId('element').value;
    const forward = state.mode === 'forward';
    if(querySpecies!==state.selected||queryMode!==state.mode) {
      const source=forward?recipes.parent(state.selected):recipes.child(state.selected);
      orderedRows=source.slice().sort((a,b)=>forward ? partner(a)-partner(b)||a[2]-b[2] : a[0]-b[0]||a[1]-b[1]);
      querySpecies=state.selected;queryMode=state.mode;
    }
    const matches=q?species.map((_,i)=>matchScore(i,q)>0):null;
    state.rows=!q&&!type?orderedRows:orderedRows.filter(row=>{
      const left=forward?partner(row):row[0], right=forward?row[2]:row[1];
      if(matches && !matches[left] && !matches[right]) return false;
      return !type || species[right].types.includes(type) || (!forward && species[left].types.includes(type));
    });
    state.page = Math.min(state.page, Math.max(1,Math.ceil(state.rows.length/resultsPerPage(state.rows.length))));
    renderResult();
  }
  function pairMember(index,side,role,sex='',showTypes=false) {
    const pal = species[index];
    const gender=sex ? `<span class="pair-sex">${who(sex)}</span>` : '';
    const elements=showTypes ? `<span class="pair-elements">${typeSet(pal)}</span>` : '';
    return `<a class="pair-member pair-map-link" data-member="${side}" href="${spawnHref(index)}" aria-label="${role}${escape(pal.zh)}${sex ? '，'+who(sex) : ''}，查看刷新位置" title="查看${escape(pal.zh)}的刷新位置">${listAvatars[index]}<div class="pair-details"><div class="pair-title"><strong class="pair-name">${escape(pal.zh)}</strong>${gender}</div><div class="pair-meta"><span class="pair-dex">${escape(dex(pal))}</span>${elements}</div></div></a>`;
  }
  function renderRow(row) {
    const forward = state.mode === 'forward';
    const [a,b,c,aSex,bSex] = row;
    const isA = a === state.selected;
    const first=forward ? partner(row) : a, second=forward ? c : b;
    const firstSex=forward ? (isA ? bSex : aSex) : aSex;
    const firstMember=pairMember(first,'first',forward ? '配种伙伴' : '亲代 A',firstSex);
    const secondMember=pairMember(second,'second',forward ? '孵化子代' : '亲代 B',forward ? '' : bSex,forward);
    const rule=forward && aSex ? `<small class="pair-sex-rule">当前亲代需为${who(isA ? aSex : bSex)}</small>` : '';
    return `<li class="pair-recipe" data-pair data-first="${first}" data-second="${second}" data-child="${c}">${firstMember}<span class="pair-operator">${icon(forward ? 'arrow' : 'plus')}</span>${secondMember}${rule}</li>`;
  }
  function pageBtn(action,label,glyph,disabled) {
    return `<button type="button" data-page="${action}" title="${label}" aria-label="${label}" ${disabled ? 'disabled' : ''}>${icon(glyph)}</button>`;
  }
  // Cache only the current parent / target, so revisiting filters and pages
  // does not accumulate a DOM node for every recipe in the game.
  const rowCache=new Map();
  let cacheSpecies,cacheMode;
  function renderPairList(rows) {
    if(cacheSpecies!==state.selected||cacheMode!==state.mode) {
      rowCache.clear();cacheSpecies=state.selected;cacheMode=state.mode;
    }
    const missing=rows.filter(row=>!rowCache.has(row));
    if(missing.length) {
      const template=document.createElement('template');
      template.innerHTML=missing.map(renderRow).join('');
      Array.from(template.content.children).forEach((element,i)=>rowCache.set(missing[i],element));
    }
    const list=byId('pair-rows');
    list.classList.toggle('long-list',rows.length>250);
    const nodes=rows.map(row=>rowCache.get(row));
    const prefix=list.childElementCount;
    if(prefix && prefix<=nodes.length && [...list.children].every((node,i)=>node===nodes[i])) {
      if(prefix<nodes.length) list.append(...nodes.slice(prefix));
    } else list.replaceChildren(...nodes);
    fillPortraits(list);
  }
  function renderResult() {
    const total = state.rows.length, pageSize = resultsPerPage(total);
    const start = (state.page-1)*pageSize, maxPage = Math.ceil(total/pageSize);
    byId('result-count').textContent = format(total);
    byId('pair-list-start').hidden = !total;
    renderPairList(state.rows.slice(start,start+pageSize));
    const empty = byId('empty');
    empty.hidden = total > 0;
    if (!total) {
      const selected = species[state.selected];
      const filtered = byId('filter').value.trim() !== '' || byId('element').value !== '';
      const message = !selected ? '请在图鉴中输入名称或编号。' : filtered ? '可以更换筛选词或属性。' : selected.reason || '此数据版本没有收录该帕鲁的对应配方。';
      empty.innerHTML = `<div class="no-results"><span class="empty-icon">${icon('egg')}</span><strong>${!selected ? '尚未选择帕鲁' : filtered ? '没有符合筛选条件的组合' : '没有配种组合'}</strong>${escape(message)}${filtered && selected ? '<br><button type="button" id="reset-filter">清除筛选</button>' : ''}</div>`;
    }
    const pages = byId('pages');
    pages.hidden = !total;
    const navigation = maxPage > 1 ? `<div class="page-controls">${pageBtn('first','第一页','first',state.page===1)}${pageBtn('prev','上一页','left',state.page===1)}<span class="page-indicator">${state.page} / ${maxPage}</span>${pageBtn('next','下一页','right',state.page===maxPage)}${pageBtn('last','最后一页','last',state.page===maxPage)}</div>` : '';
    pages.innerHTML = total ? `<span>显示 ${start+1}–${Math.min(start+pageSize,total)} 条，共 ${format(total)} 条</span>${navigation}` : '';
  }
  function catalogSearch() {
    const q = normalize(byId('pal-search').value);
    const scores = q ? allSpecies.map(index => matchScore(index,q)) : null;
    state.visible = scores ? allSpecies.filter(index => scores[index]).sort((a,b) => scores[b]-scores[a] || a-b) : allSpecies;
    const first = state.visible[0] ?? null;
    const inList = state.visible.includes(state.selected);
    const keep = inList && (!scores || scores[state.selected] === scores[first]);
    select(keep ? state.selected : first);
    byId('species-list').scrollTop = 0;
  }
  byId('pal-search').addEventListener('input', event => { if (!event.isComposing) catalogSearch(); });
  byId('pal-search').addEventListener('compositionend',catalogSearch);
  byId('pal-search').addEventListener('keydown',event => {
    if (event.isComposing || event.keyCode === 229 || !state.visible.length) return;
    if (event.key === 'Enter' || event.key === 'ArrowDown') {
      event.preventDefault();
      if (event.key === 'Enter') select(state.visible[0]);
      byId('species-list').querySelector('.selected')?.focus();
    }
  });
  byId('species-list').addEventListener('click',event => {
    const button = event.target.closest('[data-species]');
    if (!button) return;
    select(Number(button.dataset.species));
    byId('species-list').querySelector('.selected')?.focus({preventScroll:true});
  });
  byId('species-list').addEventListener('keydown',event => {
    if (!['ArrowDown','ArrowUp','Home','End'].includes(event.key)) return;
    event.preventDefault();
    const last = state.visible.length - 1;
    const current = state.visible.indexOf(state.selected);
    const next = {ArrowDown:Math.min(last,current+1),ArrowUp:Math.max(0,current-1),Home:0,End:last}[event.key];
    select(state.visible[next]);
    byId('species-list').querySelector('.selected')?.focus();
  });
  for (const mode of ['forward','reverse']) byId(mode).addEventListener('click',() => {
    if (state.mode === mode) return;
    state.mode = mode;
    for (const id of ['forward','reverse']) {
      byId(id).classList.toggle('active',id === mode);
      byId(id).setAttribute('aria-pressed',String(id === mode));
    }
    resetFilter(); updateSelection(); runQuery();
  });
  function filterResults() { state.page=1; runQuery(); }
  byId('filter').addEventListener('input', filterResults);
  byId('element').addEventListener('change', filterResults);
  byId('empty').addEventListener('click',event => {if (event.target.closest('#reset-filter')) {resetFilter(); runQuery(); byId('filter').focus();}});
  byId('pages').addEventListener('click',event => {
    const button = event.target.closest('[data-page]');
    if (!button || button.disabled) return;
    const current = state.page;
    const last = Math.ceil(state.rows.length/resultsPerPage(state.rows.length));
    const action = button.dataset.page;
    state.page = {first:1,prev:Math.max(1,current-1),next:Math.min(last,current+1),last}[action];
    renderResult();
    const listStart = byId('pair-list-start');
    listStart.focus({preventScroll:true});
    if (listStart.getBoundingClientRect().top < 0) listStart.scrollIntoView({block:'start'});
  });
  function sourceLink(source) {
    return `<a href="${escape(source.url)}" target="_blank" rel="noopener noreferrer">${escape(source.name)} ↗</a>`;
  }
  const modal = byId('about');
  const traitData = JSON.parse(byId('trait-data').textContent);
  function renderAboutIntro() {
    byId('about-body').innerHTML =
    `<h3>配种表</h3><p>游戏版本 ${escape(meta.gameVersion)} · PalCalc ${escape(meta.palcalcVersion)}，数据核对日期 ${escape(meta.checked)}。</p><p>收录 ${format(species.length)} 个物种及形态，其中 ${format(meta.eligibleSpecies)} 个可作为亲代。${format(meta.parentCount)} 组亲代搭配，对应 ${format(recipes.count)} 条常规孵化结果。</p>` +
    '<h3>怎样查看结果</h3><p>在「选择帕鲁」中输入名称或编号。「查配种伙伴」列出另一只亲代和孵出的子代。「反查双亲」列出可孵出所选帕鲁的全部双亲组合。</p><p>配种需要一雌一雄。性别影响子代时，表中会注明双方性别。表内记录常规孵化结果，随机突变可能产生其他子代。</p>' +
    '<h3>网上来源</h3><p>'+meta.sources.map(sourceLink).join('<br>')+'</p>'+
    '<h3>被动词条</h3><p>本版收录 '+traitData.traits.length+' 个被动词条。目标配种优先查找亲代继承路线，路线会标出每一代要留下的词条。</p><p>'+traitData.sources.map(sourceLink).join('<br>')+'</p>'+
    '<h3>图像与授权</h3><p>《幻兽帕鲁》角色及宣传画面归 Pocketpair 所有，《泰拉瑞亚》联动角色归各自权利人所有。图标来自 PalCalc 与 PalDB。'+
    sourceLink({name:'横幅画面出处',url:'__BANNER_SOURCE__'})+'</p>'+
    '<details><summary>开源数据许可</summary>'+meta.licenses.map(license => `<pre>${escape(license.name)}\n${escape(license.text)}</pre>`).join('')+'</details>';
  }
  let mapDataset, spawnDataset, mapImages, detailDataset, itemDataset, itemImages, ranchDataset, relicDataset;
  function readRanch() {
    return ranchDataset ??= JSON.parse(byId('ranch-data').textContent);
  }
  function readRelics() {
    return relicDataset ??= JSON.parse(byId('relic-data').textContent);
  }
  function readMapData() {
    return mapDataset ??= JSON.parse(byId('map-data').textContent);
  }
  function readSpawnData() {
    return spawnDataset ??= JSON.parse(byId('spawn-data').textContent);
  }
  function readMapImages() {
    return mapImages ??= JSON.parse(byId('map-images').textContent);
  }
  function readDetails() {
    return detailDataset ??= JSON.parse(byId('pal-details-data').textContent);
  }
  function readItems() {
    return itemDataset ??= JSON.parse(byId('item-catalog-data').textContent);
  }
  function itemIcon(id) {
    itemImages ??= JSON.parse(byId('item-icons').textContent);
    return itemImages.images[itemImages.ids[id]] || '';
  }
  const desktopTables=createAtlasDataLoader();
  if(jadeReleases.desktop){
    document.addEventListener('jade-load-install',()=>desktopTables.load('code-updates').catch(()=>{}));
  }
  bindExternalFooter(desktopTables);
  function openAbout() {
    if(desktopTables.wait('about',openAbout,'about')) return;
    if (!byId('about-map-data')) {
      renderAboutIntro();
      const data=readMapData();
      const spawning=readSpawnData();
      const details=readDetails(), items=readItems();
      const info=document.createElement('section');
      info.id='about-map-data';
      const licenses=[...new Map([...data.licenses,...spawning.licenses].map(item=>[item.text,item])).values()];
      info.innerHTML='<h3>世界地图</h3><p>地形图、资源图标和头目头像已内置。地图画面及游戏内容归 Pocketpair 所有。</p><p>'+
        data.sources.map(sourceLink).join('<br>')+'</p>'+
        '<h3>帕鲁刷新位置</h3><p>版本 '+escape(spawning.version)+' · '+format(spawning.meta.locations)+' 处刷新位置。'+escape(spawning.meta.scope)+'</p><p>'+
        escape(spawning.meta.levelNote)+'</p><p>'+spawning.sources.map(sourceLink).join('<br>')+'</p>'+
        (licenses.length?'<details><summary>地图数据许可</summary>'+licenses.map(item=>'<pre>'+escape(item.name)+'\n'+escape(item.text)+'</pre>').join('')+'</details>':'')+
        '<h3>帕鲁详情</h3><p>'+details.sources.map(([name,url])=>sourceLink({name,url})).join('<br>')+'</p>'+
        '<h3>物品图鉴</h3><p>收录 '+format(items.items.length)+' 个游戏物品记录。商店售价与货单按游戏表关联，地图坐标为近似值。</p><p>'+
        [sourceLink({name:'PalDB · 物品表',url:items.sources.items}),sourceLink({name:'PalDB · 商人货单',url:items.sources.shops}),
          sourceLink({name:'PalSphere · 物品与配方目录',url:items.sources.recipes}),sourceLink({name:'PalSphere 数据许可（个人、教育及非商业使用）',url:'https://palsphere.app/about#data-license'})].join('<br>')+'</p>'+
        '<h3>遗物转换与牧场</h3><p>转换概率表示每处理一件遗物，至少一次获得指定产物的机会。牧场显示每次放牧产出时的物品、数量及概率，可按有效牧场工作适应性等级查询。</p><p>'+
        [sourceLink({name:'古代文明遗物转换器',url:'https://paldb.cc/cn/Ancient_Relic_Recycler'}),sourceLink({name:'牧场产物',url:'https://paldb.cc/cn/Farming'})].join('<br>')+'</p>';
      byId('about-body').append(info);
    }
    modal.showModal();
  }
  byId('about-button').addEventListener('click',openAbout);
  byId('about-close').addEventListener('click',() => modal.close());
  modal.addEventListener('click',event => {
    const box = modal.getBoundingClientRect();
    if (event.target === modal && (event.clientX < box.left || event.clientX > box.right || event.clientY < box.top || event.clientY > box.bottom)) modal.close();
  });
  const stockButton = byId('lookup-warehouse-toggle');
  const worlds=createPalWorlds({species,traitData,readPassives:()=>readDetails().passives,validatePlan:plan=>validateBreedingPlan(plan,species,traitData)});
  const inventory=createPalInventory({species,traitData,storage:worlds.storage,events:worlds.events});
  const surgeryBook=createSurgeryCollection(()=>readDetails().passives,worlds.storage,worlds.events);
  const shared={species,traitData,inventory,surgeryBook,worlds,readDetails,readSpawnData,avatar,dex,icon,escape,normalize,matchScore,spawnLink,spawnHref};
  let planner,collection,offspring;
  function getPlanner() {
    return planner ??= initTraitPlanner({...shared,readPairs:recipes.all,solveTarget:createTraitRunner(traitData)});
  }
  function editIndividual(method,...args) {
    const worldId=worlds.active().id;
    function open() {
      if(worldId!==worlds.active().id || desktopTables.wait('editor',open,'editor')) return;
      getPlanner()[method](...args);
    }
    open();
  }
  function setResultPageSize(value) {
    pageSizeValue = pageSizeChoices.includes(value) ? value : '30';
    for (const select of pageSizeControls) select.value = pageSizeValue;
    state.page = 1;
    renderResult();
    offspring?.resetPage();
  }
  for (const select of pageSizeControls) select.addEventListener('change',() => {
    setResultPageSize(select.value);
    try { localStorage.setItem(pageSizeKey,pageSizeValue); }
    catch { /* Storage may be disabled in a local-file browser. */ }
  });
  window.addEventListener('storage',event => {
    if (event.key === pageSizeKey || event.key === null) setResultPageSize(event.newValue);
  });

  function updateStockButton() {
    const pal=species[state.selected];
    stockButton.hidden=!pal;
    if (!pal) return;
    const count=inventory.count(state.selected);
    const label=count ? '再加一只' : '加入仓库';
    stockButton.innerHTML=icon('plus')+label;
    stockButton.title=pal.zh+'，仓库已有 '+count+' 只';
    stockButton.setAttribute('aria-label',pal.zh+'，'+label);
  }
  stockButton.addEventListener('click',()=>editIndividual('addIndividual',state.selected));
  inventory.subscribe(()=>{
    updateStockButton();
    if(collection && !byId('warehouse-view').hidden) collection.refresh();
    if(planner && (!byId('planner-view').hidden || byId('tp-profile-dialog').open)) planner.refresh();
    if(offspring && !byId('offspring-view').hidden) offspring.refresh();
  });

  const views={
    lookup:['帕鲁配种查询','选一只帕鲁，查找配种伙伴，或反查它的双亲。'],
    warehouse:['配种仓库','查看每只帕鲁的词条、性别和个体名称。'],
    offspring:['可配出的帕鲁','从仓库里的个体出发，按步数查看能孵出的子代。'],
    planner:['目标配种','选定帕鲁和被动词条，用仓库里的亲代规划繁育路线。']
  };
  const rail=byId('section-rail'), shell=byId('shell-pages'), openRail=byId('rail-open');
  const narrowRail=window.matchMedia('(max-width:860px)');
  const sections={breeding:'配种手册',map:'世界地图',paldex:'帕鲁图鉴',enemies:'单位图鉴',details:'帕鲁详情',work:'帕鲁工作',items:'物品图鉴',guide:'冒险指南',settings:'设置'};
  const workViews=['suitability','facilities','production','efficiency','ranch','relic','expeditions'];
  const guideTitles={elements:'属性克制',status:'异常状态',journals:'探索手记',gathering:'野外采集',loot:'探索奖励表',quests:'任务详情'};
  const guideViews=Object.keys(guideTitles);
  const unitTitles={enemies:'敌人图鉴',merchants:'商人图鉴',neutrals:'其他中立单位'};
  const detailTitles={active:'主动技能',passive:'被动技能',partner:'伙伴技能',fruits:'技能果实',surgery:'帕鲁手术台',stats:'属性值计算',capture:'捕捉概率'};
  let worldMap, spawnMap, palDetails, statCalc, captureCalc, itemCatalog, palDex, relicGuide, ranchGuide, lastSpawnKey=species[0].key;
  let pendingMapJump,facilityGuide,efficiencyCalc,expeditionGuide,expeditionData,combatGuide,journalGuide,partnerGuide,enemyGuide;
  let adventureGuide,resourceGuide;
  const readExpeditions=()=>expeditionData??=JSON.parse(byId('expedition-data').textContent);
  const mapOriginName=hash=>{
    const [page,part]=(hash||'').slice(1).split('/');
    if(page==='guide') return ({journals:'手记',gathering:'采集',quests:'任务',loot:'探索奖励'})[part]||'';
    if(page==='work') return ({expeditions:'远征',production:'据点生产'})[part]||'';
    return ({items:'物品',enemies:'敌人',merchants:'商人',neutrals:'中立单位'})[page]||'';
  };
  const palByKey=new Map(species.map((pal,i)=>[pal.key,i]));
  function syncSpawnUrl(key) {
    if (!palByKey.has(key)) return;
    lastSpawnKey=key;
    const next=spawnHref(palByKey.get(key));
    byId('nav-spawns').href=next;
    if (/^#spawns(?:\/|$)/.test(location.hash)) {
      if (location.hash!==next) history.replaceState(history.state,'',next);
      byId('section-map').href=next;
    }
  }
  function setRail(open,restore=false) {
    const visible=open && narrowRail.matches;
    rail.classList.toggle('is-open',visible);
    byId('rail-shade').hidden=!visible;
    shell.inert=visible;
    openRail.setAttribute('aria-expanded',String(visible));
    if (visible) rail.querySelector('.section-active').focus();
    else if (restore && narrowRail.matches) openRail.focus();
  }
  openRail.addEventListener('click',()=>setRail(true));
  initWorldManager({
    worlds,inventory,beforeOpen:()=>setRail(false),prepareImport:()=>desktopTables.load('pal-details-data'),
    restoreFocus:button=>{
      if(narrowRail.matches && rail.contains(button)) openRail.focus();
      else if(button.isConnected && button.offsetWidth) button.focus({preventScroll:true});
    }
  });
  worlds.subscribe(change=>{
    if(change.type!=='switch') return;
    collection?.resetWorld();offspring?.closeDialog();planner?.closeDialog();
    updateStockButton();
    if(offspring && !byId('offspring-view').hidden) offspring.refresh();
  });
  byId('rail-close').addEventListener('click',()=>setRail(false,true));
  byId('rail-shade').addEventListener('click',()=>setRail(false,true));
  rail.querySelector('.section-links').addEventListener('click',()=>setRail(false,true));
  narrowRail.addEventListener('change',()=>setRail(false));
  rail.addEventListener('keydown',event=>{
    if (!rail.classList.contains('is-open') || modal.open) return;
    if (event.key==='Escape') {event.preventDefault();setRail(false,true);}
    if (event.key==='Tab') {
      const controls=[...rail.querySelectorAll('button,a[href]')];
      if (event.shiftKey && document.activeElement===controls[0]) {event.preventDefault();controls.at(-1).focus();}
      else if (!event.shiftKey && document.activeElement===controls.at(-1)) {event.preventDefault();controls[0].focus();}
    }
  });
  function detailTabLinks(key) {
    for(const tab of Object.keys(detailTitles)) {
      byId('nav-'+tab).href='#details/'+tab+(tab!=='partner'&&palByKey.has(key)?'/'+encodeURIComponent(key):'');
    }
  }
  // Mark only entries belonging to this handbook tab. A direct link can have
  // browser history outside the handbook, including other local documents.
  const backButton=byId('atlas-back'),scrollPositions=new Map();
  const scrollLists=()=>document.querySelectorAll('#pk-list,#fc-list,#ex-list,#enemy-list,#ad-list,[data-rg-list],#wm-list,#sm-list,#species-list,[data-ic="scroll"]');
  let depth=Number.isInteger(history.state?.atlasDepth)?history.state.atlasDepth:0;
  let goingBack=false,restoreScroll=null;
  history.scrollRestoration='manual';
  function markEntry() {
    history.replaceState({...history.state,atlasDepth:depth},'',location.href);
    goingBack=false;backButton.disabled=depth===0;
  }
  function goBack() {
    const dialog=document.activeElement?.closest('dialog[open]')||document.querySelector('dialog[open]');
    if(dialog){
      if(dialog.dispatchEvent(new Event('cancel',{cancelable:true})))dialog.close();
      return;
    }
    if(rail.classList.contains('is-open')){setRail(false,true);return;}
    if(depth<=0||goingBack)return;
    goingBack=true;backButton.disabled=true;history.back();
  }
  markEntry();
  backButton.addEventListener('click',goBack);
  document.addEventListener('keydown',event=>{
    if(event.key==='ArrowLeft'&&event.altKey&&!event.ctrlKey&&!event.metaKey&&!event.shiftKey){
      event.preventDefault();goBack();
    }
  });
  function showActiveView() {
    let hash=location.hash.slice(1);
    const migrated=hash.replace(/^details\/work(?=\/|$)/,'work/suitability').replace(/^(ranch|relic)(?=\/|$)/,'work/$1');
    if(migrated!==hash){hash=migrated;history.replaceState(history.state,'','#'+hash);}
    const [page,part,third,fourth,fifth]=hash.split('/').map(segment=>{
      try {return decodeURIComponent(segment);} catch {return '';}
    });
    const onWork=page==='work',onGuide=page==='guide',onPartner=page==='details'&&part==='partner';
    const [subpage,arg]=onWork||onGuide||onPartner?[third,fourth]:[part,third];
    const view=onWork?(workViews.includes(part)?part:'suitability'):
      onGuide?(guideViews.includes(part)?part:'elements'):onPartner?'partner':page==='settings'&&part==='updates'?'updates':
      (Object.hasOwn(views,page)||Object.hasOwn(unitTitles,page)||['map','spawns','details','paldex','items','settings'].includes(page)?page:'lookup');
    if(desktopTables.wait(view+'/'+(subpage||''),showActiveView)) return;
    if(restoreScroll!==null){
      const target=restoreScroll,entry=depth;restoreScroll=null;
      requestAnimationFrame(()=>{
        if(entry!==depth)return;
        for(const [list,top] of target.lists)list.scrollTop=top;
        window.scrollTo({top:target.top,behavior:'instant'});
      });
    }
    const onSpawns=view==='spawns';
    const onMap=onSpawns || view==='map';
    const onUnits=Object.hasOwn(unitTitles,view);
    const section=onWork?'work':onGuide?'guide':onPartner?'details':onMap?'map':onUnits?'enemies':view==='updates'?'settings':Object.hasOwn(views,view)?'breeding':view;
    for (const id of Object.keys(sections)) {
      const active=id===section, link=byId('section-'+id);
      link.classList.toggle('section-active',active);
      if(active) {
        link.setAttribute('aria-current','true');
        if(onWork||onGuide) link.href='#'+(part&&(onWork?workViews:guideViews).includes(part)?hash:onWork?'work/suitability':'guide/elements');
        else if(section==='details') link.href='#'+(Object.hasOwn(detailTitles,part)?hash:'details/active');
        else if(section==='settings') link.href=view==='updates'?'#settings/updates':'#settings/skins';
        else link.href='#'+(view===page && section!=='paldex'?hash:view);
      } else link.removeAttribute('aria-current');
    }
    setRail(false);
    planner?.closeDialog();collection?.closeDialog();offspring?.closeDialog();
    if (view!=='map') worldMap?.deactivate();
    if (!onSpawns) spawnMap?.deactivate();
    if (view!=='details' && view!=='suitability') palDetails?.deactivate();
    document.body.classList.toggle('map-mode',onMap);
    const navGroups={breeding:'breeding-tabs',map:'map-tabs',items:'item-tabs',work:'work-tabs',details:'pd-tabs',guide:'guide-tabs',paldex:'dex-tabs',enemies:'enemy-tabs',settings:'settings-tabs'};
    for(const [name,id] of Object.entries(navGroups)) byId(id).hidden=name!==section;
    byId('mobile-section-title').textContent=sections[section];
    document.querySelector('.hero').hidden=section!=='breeding';
    byId('pal-details-view').hidden=view!=='details'&&view!=='suitability';
    byId('paldex-view').hidden=view!=='paldex';
    byId('partner-skills-view').hidden=!onPartner;
    byId('combat-guide-view').hidden=view!=='elements'&&view!=='status';
    byId('journals-view').hidden=view!=='journals';
    byId('adventure-view').hidden=view!=='quests'&&view!=='loot';
    byId('gathering-view').hidden=view!=='gathering';
    byId('enemies-view').hidden=!onUnits;
    byId('settings-view').hidden=view!=='settings';
    byId('updates-view').hidden=view!=='updates';
    for (const id of [...Object.keys(views),'map','spawns','items',...workViews]) {
      if(id!=='suitability') byId(id==='efficiency'?'work-efficiency-view':id+'-view').hidden=view!==id;
    }
    const detailTab=onPartner?'partner':Object.hasOwn(detailTitles,subpage)?subpage:'active';
    const activeTab=view==='details'?detailTab:view==='settings'?'skins':view;
    const selectedKey=onPartner?subpage:arg;
    if(section==='details') detailTabLinks(selectedKey);
    const nav=byId(navGroups[section]);
    for (const link of nav.querySelectorAll('a')) {
      const active=link.id==='nav-'+activeTab;
      link.classList.toggle('current-nav',active);
      if (active) link.setAttribute('aria-current','page');
      else link.removeAttribute('aria-current');
    }
    const activeLink=byId('nav-'+activeTab);
    if(activeLink && nav.scrollWidth>nav.clientWidth) {
      const rect=activeLink.getBoundingClientRect(),bar=nav.getBoundingClientRect();
      if(rect.left<bar.left) nav.scrollLeft+=rect.left-bar.left;
      else if(rect.right>bar.right) nav.scrollLeft+=rect.right-bar.right;
    }
    if(view==='updates'){
      setPageTitle('公告与更新');
      showReleaseCenter();
      return;
    }
    if(view==='settings'){
      setPageTitle('皮肤设置');
      initSkinSettings();
      return;
    }
    if(onUnits){
      setPageTitle(unitTitles[view]);
      enemyGuide??=initEnemyGuide({data:JSON.parse(byId('enemy-data').textContent),species,portraits,itemIcon,escape});
      enemyGuide.show(subpage,view);
      return;
    }
    if(view==='gathering'||view==='production'){
      setPageTitle(view==='production'?'据点生产':guideTitles.gathering);
      resourceGuide??=initResourceGuides({data:JSON.parse(byId('resource-guides-data').textContent),itemIcon});
      resourceGuide.show(view,subpage);
      return;
    }
    if(onPartner) {
      setPageTitle('伙伴技能');
      partnerGuide??=initPartnerSkills({data:JSON.parse(byId('partner-skills-data').textContent),species,avatar,listAvatars,fillPortraits,onPalChange:detailTabLinks,escape,normalize});
      partnerGuide.show(subpage);return;
    }
    if(onGuide) {
      setPageTitle(guideTitles[view]);
      if(view==='loot'||view==='quests'){
        adventureGuide??=initAdventureTables({data:JSON.parse(byId('adventure-data').textContent),itemIcon,escape});
        adventureGuide.show(view,subpage,arg,fifth);
      }else if(view==='journals'){
        journalGuide??=initJournals({data:JSON.parse(byId('journals-data').textContent),escape});
        journalGuide.show(subpage);
      }else{
        combatGuide??=initCombatGuide({data:JSON.parse(byId('combat-guide-data').textContent),escape});
        combatGuide.show(view,subpage);
      }
      return;
    }
    if(view==='expeditions') {
      setPageTitle('远征所');
      expeditionGuide??=initExpeditions({data:readExpeditions(),items:readItems().items,itemIcon});
      expeditionGuide.show(subpage);return;
    }
    if(view==='facilities') {
      setPageTitle('设施');
      facilityGuide??=initFacilities({data:JSON.parse(byId('facilities-data').textContent),items:readItems().items,itemIcon,escape,normalize});
      facilityGuide.show(subpage);
      return;
    }
    if(view==='efficiency') {
      setPageTitle('工作效率');
      efficiencyCalc??=initWorkEfficiency({species,traits:traitData.traits,profiles:JSON.parse(byId('paldex-index-data').textContent).profiles,
        inventory,escape,data:JSON.parse(byId('work-efficiency-data').textContent)});
      efficiencyCalc.show(subpage,arg);
      return;
    }
    if(view==='paldex') {
      setPageTitle('帕鲁图鉴');
      palDex ??= initPalDex({...shared,root:byId('paldex-view'),typeSet,itemIcon,listAvatars,fillPortraits,
        data:JSON.parse(byId('paldex-data').textContent),index:JSON.parse(byId('paldex-index-data').textContent),
        detailData:readDetails(),ranchData:readRanch()});
      palDex.show(subpage);
      return;
    }
    if(view==='ranch') {
      setPageTitle('牧场产物');
      ranchGuide ??= initRanchGuide({...shared,root:byId('ranch-view'),data:readRanch(),items:readItems().items,itemIcon,typeSet});
      ranchGuide.show(subpage==='item'?arg:subpage,subpage==='item'?'item':'pal');
      return;
    }
    if(view==='relic') {
      setPageTitle('遗物转换');
      relicGuide ??= initRelicGuide({...shared,root:byId('relic-view'),data:readRelics(),items:readItems().items,itemIcon});
      relicGuide.show(subpage==='item'?arg:subpage,subpage==='item'?'item':'input');
      return;
    }
    if (view==='details'||view==='suitability') {
      const tab=view==='suitability'?'work':Object.hasOwn(detailTitles,subpage)?subpage:'active';
      setPageTitle(tab==='work'?'工作适应性':detailTitles[tab]);
      palDetails ??= initPalDetails({...shared,typeSet,itemIcon,data:readDetails(),
        spawnData:tab==='work'?JSON.parse(byId('paldex-index-data').textContent):readSpawnData()});
      palDetails.show(tab,tab==='work'?subpage:arg,fourth);
      const key=palByKey.has(arg)?arg:species[0].key;
      if(tab==='stats') {
        statCalc ??= initPalStatCalc({...shared,root:byId('pal-stat-calc'),typeSet,itemIcon,portraits,
          data:JSON.parse(byId('stat-data').textContent),spawnData:readSpawnData(),detailData:readDetails()});
        statCalc.show(key);
      }
      if(tab==='capture') {
        captureCalc ??= initCaptureCalc({root:byId('pal-capture-calc'),species,itemIcon,escape,spawnSites:readSpawnData().bySpecies,data:JSON.parse(byId('capture-data').textContent)});
        captureCalc.show(key);
      }
      return;
    }
    if (view==='items') {
      setPageTitle('物品图鉴');
      itemCatalog ??= initItemCatalog({
        root:byId('items-view'),data:readItems(),itemIcon,species,ranchData:readRanch(),relicData:readRelics(),expeditionData:readExpeditions(),
        navigate:(id,shops=false)=>{location.hash=id?'#items/'+encodeURIComponent(id)+(shops?'/shops':''):'#items';},
        openMapPoint:point=>{
          if (!point?.id) return;
          pendingMapJump={from:location.hash,to:'#map/point/'+encodeURIComponent(point.id)};
          location.hash=pendingMapJump.to;
        }
      });
      itemCatalog.show(subpage,arg);
      return;
    }
    if (onSpawns) {
      const key=palByKey.has(subpage)?subpage:lastSpawnKey;
      setPageTitle('帕鲁刷新位置');
      spawnMap ??= initSpawnMap({...shared,typeSet,portraits,data:readSpawnData(),mapData:readMapData(),images:readMapImages(),onSpeciesChange:syncSpawnUrl});
      spawnMap.selectSpecies(key);spawnMap.refresh();syncSpawnUrl(key);
      return;
    }
    if (view==='map') {
      setPageTitle('世界地图');
      const returnLink=byId('wm-item-back'), origin=history.state?.atlasMapOrigin||history.state?.itemOrigin;
      returnLink.hidden=!(['point','category','coords'].includes(subpage) && mapOriginName(origin));
      if (!returnLink.hidden) {returnLink.href=origin;byId('wm-origin-label').textContent='返回'+mapOriginName(origin);}
      worldMap ??= initWorldMap({data:readMapData(),images:readMapImages(),species,portraits,worlds});
      worldMap.refresh();
      if (subpage==='point' && arg) worldMap.focusPoint(arg);
      if (subpage==='category' && arg) worldMap.focusCategory(arg,fourth,fifth);
      if (subpage==='coords' && arg && fourth && fifth) worldMap.focusCoordinates(arg,Number(fourth),Number(fifth));
      return;
    }
    const [title,description]=views[view];
    if(view==='lookup') {
      const index=palByKey.has(subpage)?palByKey.get(subpage):state.selected;
      if(palByKey.has(subpage) && !state.visible.includes(index)) {
        byId('pal-search').value='';
        state.visible=allSpecies;
      }
      if(!catalogRows||state.selected!==index) select(index,false);
    }
    document.querySelector('.hero').dataset.page=view;
    byId('page-title').textContent=title;
    byId('page-description').textContent=description;
    setPageTitle(title);
    if(view==='warehouse') {
      if(collection) collection.refresh();
      else collection = initPalCollection({...shared,addIndividual:(...args)=>editIndividual('addIndividual',...args),
        editIndividual:(...args)=>editIndividual('editIndividual',...args)});
    }
    if(view==='offspring') {
      offspring ??= initOffspringPage({...shared,readPairs:recipes.all,resultsPerPage});
      offspring.refresh();
    }
    if(view==='planner') {
      if(planner) planner.refresh();
      else getPlanner();
    }
  }
  document.addEventListener('click',event=>{
    const href=event.target.closest('a[href]')?.getAttribute('href');
    if(href && /^#map\/(point|coords|category)\//.test(href) && mapOriginName(location.hash))
      pendingMapJump={from:location.hash,to:new URL(href,location.href).hash};
  });
  byId('wm-item-back').addEventListener('click',event=>{
    if (!event.ctrlKey && !event.shiftKey && !event.metaKey && !event.altKey && (history.state?.atlasMapOrigin||history.state?.itemOrigin)) {
      event.preventDefault();
      history.back();
    }
  });
  window.addEventListener('hashchange',event=>{
    if(event.newURL!==location.href)return;
    scrollPositions.set(depth,{top:window.scrollY,lists:[...scrollLists()].filter(list=>list.clientHeight>0).map(list=>[list,list.scrollTop])});
    const savedDepth=history.state?.atlasDepth;
    if(Number.isInteger(savedDepth)){
      depth=savedDepth;restoreScroll=scrollPositions.get(depth)??null;
    }else{
      depth++;scrollPositions.delete(depth);restoreScroll=null;
    }
    markEntry();
    const previous=new URL(event.oldURL).hash;
    if (pendingMapJump?.from===previous && pendingMapJump.to===location.hash) {
      history.replaceState({...history.state,atlasMapOrigin:previous},'',location.href);
    }
    pendingMapJump=null;
    window.scrollTo({top:0,behavior:'instant'});
    showActiveView();
  });
  showActiveView();
})();

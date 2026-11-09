function initEnemyGuide({data,species,portraits,itemIcon,escape}) {
  const get = suffix => document.getElementById('enemy-' + suffix);
  const search = get('search'), group = get('group'), filter = get('filter');
  const list = get('list'), detail = get('detail');
  const norm = text => String(text).normalize('NFKC').toLowerCase();
  const itemName = id => data.itemNames[id] || id;
  const units = [...data.enemies,...(data.people || [])];
  const byId = new Map(units.map(unit => [unit.id,unit]));
  for (const unit of data.people || []) for(const id of unit.sourceIds || []) byId.set(id,unit);
  const categories = {
    enemies:['敌人图鉴','查找敌人或掉落物','名称、阵营或掉落物'],
    merchants:['商人图鉴','查找商人、商品或在售帕鲁','商人、地点、商品或帕鲁'],
    neutrals:['中立单位图鉴','查找人物或地点','人物名称、地点或掉落物']
  };
  const tabState = new Map();
  let category;
  const pals = new Map(species.map(pal => [pal.key, pal]));
  const route = id => '#'+(byId.get(id)?.category || 'enemies')+'/' + encodeURIComponent(id);
  const link = (href, label) => '<a href="' + escape(href) + '">' + escape(label) + '</a>';
  const tiers = enemy => enemy.dropTiers || [];
  const allDrops = enemy => [...(enemy.drops || []), ...tiers(enemy).flatMap(tier => tier.drops)];
  const products = unit => (unit.shops || []).flatMap(id => data.shopLists[id].rows);
  const terms = new Map(units.map(enemy => [enemy.id, norm([enemy.name,enemy.name.replace(/\s/g,''),enemy.id,enemy.group,enemy.variant,
    ...(enemy.aliases || []),...(enemy.sourceIds || []),...(enemy.palStock?.pals || []).map(id=>pals.get(id)?.zh),
    ...(enemy.requests || []).flatMap(r=>[r.item && itemName(r.item),r.pal && pals.get(r.pal)?.zh,...r.rewards.map(([id])=>itemName(id))]),
    ...products(enemy).flatMap(([id,,,currency])=>[id,itemName(id),itemName(currency)]),
    enemy.weapon,...(enemy.points || []).map(p => p.name), ...allDrops(enemy).map(([id]) => itemName(id)),
    enemy.firstReward && itemName(enemy.firstReward),enemy.summonItem && itemName(enemy.summonItem)].filter(Boolean).join(' '))]));
  let active, visible = [], lastFilter, lastRendered;
  function switchTab(value) {
    if(category===value) return;
    if(category) tabState.set(category,{query:search.value,group:group.value,filter:filter.value,active});
    category=value;
    const state=tabState.get(category);
    const current=units.filter(unit=>(unit.category || 'enemies')===category);
    const [title,searchLabel,placeholder]=categories[category];
    get('heading').textContent=title; get('search-label').textContent=searchLabel;
    search.placeholder=placeholder;
    get('summary').textContent=current.length+' 个条目';
    group.replaceChildren(new Option('全部分类',''));
    for (const name of new Set(current.map(u=>u.group))) group.add(new Option(name,name));
    filter.replaceChildren(new Option('全部资料',''),new Option('有掉落记录','loot'),new Option('可在地图定位','map'));
    if(category==='merchants') {
      filter.add(new Option('出售物品','items'));filter.add(new Option('出售帕鲁','pals'));
    }
    search.value=state?.query || '';group.value=state?.group || '';filter.value=state?.filter || '';
    active=state?.active;lastFilter=null;lastRendered=null;
  }

  function portrait(enemy) {
    const src = data.icons[enemy.icon] || portraits[pals.get(enemy.palKey)?.en];
    return '<span class="enemy-portrait" aria-hidden="true">' + (src
      ? '<img src="' + escape(src) + '" loading="lazy" alt="" width="36" height="36">'
      : escape(enemy.name.slice(0,1))) + '</span>';
  }
  function lootCard(id, note) {
    const src = itemIcon(id);
    return '<a class="enemy-reward" href="#items/' + encodeURIComponent(id) + '">' +
      (src ? '<img src="' + escape(src) + '" loading="lazy" alt="">' : '') + '<span><b>' +
      escape(itemName(id)) + '</b>' + (note ? '<small>' + escape(note) + '</small>' : '') + '</span></a>';
  }
  function lootRows(rows) {
    return '<div class="enemy-rewards">' + rows.map(([id, lo, hi, percent, form]) => {
      const amount = lo == null ? '' : '×' + lo + (hi != null && hi !== lo ? '–' + hi : '');
      const chance = percent == null ? '' : percent + '%';
      return lootCard(id,[form,amount,chance].filter(Boolean).join(' · '));
    }).join('') + '</div>';
  }
  function locationLink(point) {
    let label = point.name || '地图位置';
    if (Number.isFinite(point.x) && Number.isFinite(point.y)) label += '（' + point.x + '，' + point.y + '）';
    if (point.level) label += ' · Lv. ' + point.level;
    return link('#map/point/' + encodeURIComponent(point.id),label);
  }
  function renderShops(unit) {
    const list=(unit.shops || []).map(id=>{
      const shop=data.shopLists[id], cards=shop.rows.map(([product,qty,cost,currency,once])=>
        '<div class="unit-stock">'+lootCard(product,'每份 ×'+qty+(once?' · 限购一次':''))+
        '<div class="unit-price">'+link('#items/'+encodeURIComponent(currency),itemName(currency))+
        ' ×'+Number(cost).toLocaleString('zh-CN')+'</div></div>').join('');
      return '<h4>'+escape(shop.name)+(unit.shops.length>1?' · '+escape(id.split('_').at(-1)):'')+'</h4>'+
        (shop.note?'<p class="enemy-note">'+escape(shop.note)+'</p>':'')+'<div class="enemy-rewards">'+cards+'</div>';
    }).join('');
    if(!list)return '';
    return '<section><h3>出售物品</h3>'+list+'</section>';
  }
  function renderPalStock(stock) {
    if(!stock)return '';
    const cards=stock.pals.map(key=>{
      const pal=pals.get(key),src=portraits[pal.en];
      return '<a class="enemy-reward" href="#paldex/'+encodeURIComponent(key)+'">'+
        (src?'<img src="'+escape(src)+'" alt="" loading="lazy">':'')+'<b>'+escape(pal.zh)+'</b></a>';
    }).join('');
    return '<section><h3>可出售帕鲁</h3><p class="enemy-note">每批从以下候选中抽取 '+stock.count+
      ' 只，等级 Lv. '+stock.min+'–'+stock.max+'。</p><div class="enemy-rewards">'+cards+'</div></section>';
  }
  function renderRequests(requests) {
    if(!requests?.length) return '';
    return '<section><h3>委托条件与报酬</h3><dl class="unit-requests">'+requests.map(row=>{
      const goal=row.pal?'展示 '+link('#paldex/'+encodeURIComponent(row.pal),pals.get(row.pal).zh):
        '交付 '+link('#items/'+encodeURIComponent(row.item),itemName(row.item))+' ×'+row.qty;
      const rewards=row.rewards.map(([id,qty])=>link('#items/'+encodeURIComponent(id),itemName(id))+' ×'+qty);
      return '<dt>'+goal+'</dt><dd>'+rewards.join('、')+'</dd>';
    }).join('')+'</dl></section>';
  }
  function renderDetail(enemy) {
    if (!enemy) {
      detail.innerHTML = '<p class="enemy-note">没有找到对应单位。可更换关键词或分类。</p>';
      return;
    }
    const variant = enemy.variant !== enemy.name && enemy.variant !== enemy.group ? enemy.variant : '';
    const kicker = [enemy.group,variant].filter(Boolean).join(' · ');
    const facts = [...new Set([enemy.level ? 'Lv. ' + enemy.level : '', enemy.weapon, enemy.detailLabel].filter(Boolean))].join(' · ');
    const section = (title,html) => '<section><h3>' + title + '</h3>' + html + '</section>';
    let loot = enemy.drops?.length ? (tiers(enemy).length ? '<h4>基础档</h4>' : '') + lootRows(enemy.drops) : '';
    for (const tier of tiers(enemy)) {
      loot += '<h4>Lv. ' + escape(tier.level) + ' 起</h4>' + lootRows(tier.drops);
    }
    if (!loot && category==='enemies') loot = '<p class="enemy-note">未收录此敌人的直接掉落记录。</p>';
    const parts = ['<div class="enemy-profile">' + portrait(enemy) + '<div><span class="enemy-kicker">' + escape(kicker) +
      '</span><h2>' + escape(enemy.name) + '</h2>' + (facts ? '<p class="enemy-meta">' + escape(facts) + '</p>' : '') +
      '</div></div>'];
    if(enemy.note) parts.push('<p class="enemy-note">'+escape(enemy.note)+'</p>');
    if (enemy.points?.length && category!=='enemies') parts.push(section('地图位置','<div class="enemy-links">' + enemy.points.map(locationLink).join('') + '</div>'));
    parts.push(renderShops(enemy),renderPalStock(enemy.palStock),renderRequests(enemy.requests));
    const quests=data.questTargets?.[enemy.id] || [];
    if(quests.length) parts.push(section('相关任务','<div class="enemy-links">'+quests.map(([id,name])=>
      link('#guide/quests/'+encodeURIComponent(id),name)).join('')+'</div>'));
    if(loot) parts.push(section(category==='enemies'?'击败掉落':'人物掉落',loot));
    if (enemy.firstReward) parts.push(section('首次胜利奖励',lootCard(enemy.firstReward)));
    if (enemy.summonItem) parts.push(section('召唤所需物品',lootCard(enemy.summonItem)));
    if (enemy.points?.length && category==='enemies') parts.push(section('地图位置','<div class="enemy-links">' + enemy.points.map(locationLink).join('') + '</div>'));
    if (enemy.palKey) parts.push(section('帕鲁资料','<div class="enemy-links">' +
      link('#paldex/' + encodeURIComponent(enemy.palKey),pals.get(enemy.palKey).zh + ' · 帕鲁图鉴') + '</div>'));
    if(enemy.work?.length) parts.push(section('工作适应性','<p class="enemy-meta">'+enemy.work.map(([name,level])=>
      escape(name)+' Lv. '+level).join('　')+'</p>'));
    detail.innerHTML = parts.join('');
  }
  function renderList() {
    const tokens = norm(search.value).trim().split(/\s+/).filter(Boolean);
    const key = JSON.stringify([tokens,group.value,filter.value]);
    if (key === lastFilter) return;
    lastFilter = key;
    visible = units.filter(enemy => (enemy.category || 'enemies')===category && (!group.value || enemy.group === group.value) &&
      (filter.value !== 'loot' || enemy.drops?.length || tiers(enemy).some(t => t.drops?.length)) &&
      (filter.value !== 'items' || enemy.shops?.length) && (filter.value !== 'pals' || enemy.palStock) &&
      (filter.value !== 'map' || enemy.points?.length) && tokens.every(t => terms.get(enemy.id).includes(t)));
    if(category==='neutrals') {
      const score=u=>u.requests?.length?2:u.points?.length?1:0;
      visible.sort((a,b)=>score(b)-score(a));
    }
    get('count').textContent = visible.length + ' 个匹配条目';
    list.innerHTML = visible.map(enemy => '<a class="enemy-choice" href="' + escape(route(enemy.id)) + '" data-enemy="' +
      escape(enemy.id) + '">' + portrait(enemy) + '<span>' + escape(enemy.name) + '<small>' +
      escape([enemy.variant || enemy.group,enemy.detailLabel].filter(Boolean).join(' · ')) + '</small></span></a>').join('');
    list.scrollTop = 0;
    lastRendered = null;
  }
  function select(enemy) {
    if (enemy && enemy.id === lastRendered) return;
    active = enemy;
    lastRendered = enemy?.id;
    list.querySelector('[aria-current]')?.removeAttribute('aria-current');
    if (enemy) list.querySelector('[data-enemy="' + CSS.escape(enemy.id) + '"]')?.setAttribute('aria-current','true');
    renderDetail(enemy);
  }
  function applyFilters() {
    renderList();
    select(visible.includes(active) ? active : visible[0]);
    history.replaceState(history.state,'',active ? route(active.id) : '#'+category);
  }
  for (const control of [search,group,filter]) control.addEventListener(control === search ? 'input' : 'change',applyFilters);
  list.addEventListener('click',event => {
    if (!event.target.closest('a') || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey) return;
    if (matchMedia('(max-width:820px)').matches) requestAnimationFrame(() => {
      detail.scrollIntoView({block:'start'});detail.focus({preventScroll:true});
    });
  });
  get('sources').innerHTML = '资料来源 ' + (data.sources || []).filter(s => /^https:\/\//.test(s.url)).map(s =>
    '<a href="' + escape(s.url) + '" target="_blank" rel="noopener noreferrer">' + escape(s.name) + '</a>').join(' ');
  return {show(id,tab='enemies') {
    const requested = byId.get(id);
    switchTab(requested ? (requested.category || 'enemies') : (Object.hasOwn(categories,tab)?tab:'enemies'));
    if(requested && tab!==category) {location.replace(route(requested.id)); return;}
    renderList();
    if (requested && !visible.includes(requested)) {
      search.value = '';group.value = '';filter.value = '';renderList();
    }
    select(id ? requested : visible.includes(active) ? active : visible[0]);
  }};
}

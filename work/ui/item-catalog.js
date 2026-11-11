function matchingItemEnemies(catalog, itemId, name) {
  const named = catalog.enemyNames?.[name] || [];
  const drops = catalog.enemyDrops?.[itemId] || [];
  const matches = drops.filter(([id, label]) => label === name || named.some(([namedId]) => namedId === id));
  return matches.length ? matches : named.length === 1 ? named : [];
}

const itemProductionAliases={'大型原油提炼机':'High-Pressure_Crude_Oil_Extractor','硬木采伐场':'Logging_Site_II','采伐场':'Logging_Site'};
const itemLootChapters={'空投补给':'SuppliesLoots','海上打捞':'PowerfulFishingMagnetLoots','废品堆':'JunkyardLoots','藏宝图':'TreasureMapLoots'};

function initItemCatalog({root, data, openMapPoint, navigate, itemIcon = () => '', species=[], relicData, ranchData,expeditionData}) {
  if (typeof root === 'string') root = document.querySelector(root);
  root = root || document.getElementById('items-view');
  const el = key => root.querySelector('[data-ic="' + key + '"]');
  const list = el('list'), detail = el('detail'), scroll = el('scroll');
  const byId = new Map(data.items.map(item => [item.id, item]));
  const ranks = ['常见', '少见', '稀有', '史诗', '传说'];
  const groupLabels = ['商人出售', '配方制作', '敌人掉落', '敌人首胜奖励', '帕鲁掉落', '头目掉落', '野外采集', '牧场生产', '遗物转换', '据点生产', '任务奖励'];
  const relicInputs=new Set((relicData?.tiers||[]).map(t=>t.id));
  const relicOutputs=new Set(relicData?.outputs||[]);
  const palNames=new Map(species.map(p=>[p.key,p.zh]));
  const palByName=new Map();
  for(const p of species) {
    if(!palByName.has(p.zh))palByName.set(p.zh,[]);
    palByName.get(p.zh).push(p);
  }
  const escapedName=s=>s.replace(/[.*+?^${}()|[\]\\]/g,'\\$&');
  const palPattern=new RegExp([...palByName.keys()].sort((a,b)=>b.length-a.length).map(escapedName).join('|')||'(?!)','g');
  const sourceData=data.acquisitionLinks||{};
  const expeditionRewards=new Map();
  for(const s of expeditionData?.stages||[]) for(const id of new Set(s.rewards.map(row=>row[0]))){
    if(!expeditionRewards.has(id))expeditionRewards.set(id,[]);
    expeditionRewards.get(id).push(s);
  }
  const ranchOutputs=new Map();
  for(const [key,pal] of Object.entries(ranchData?.pals||{})) {
    const products=new Set(pal.tiers.flat().map(drop=>drop.item));
    for(const id of products) {
      if(!ranchOutputs.has(id)) ranchOutputs.set(id,[]);
      ranchOutputs.get(id).push(key);
    }
  }
  const quick = ['PalSphere', 'Cake', 'CopperOre', 'CopperIngot', 'Wood', 'Stone', 'Pal_crystal_S', 'Wool', 'Milk', 'Egg', 'Honey', 'Flour', 'Arrow', 'Drug_Low', 'DogCoin', 'Money'];
  const priority = new Map(quick.map((id, i) => [id, i]));
  const sortZh = new Intl.Collator('zh-CN', {numeric:true});
  const escape = value => String(value ?? '').replace(/[&<>"']/g, ch => ({'&':'&amp;', '<':'&lt;', '>':'&gt;', '"':'&quot;', "'":'&#39;'}[ch]));
  const route = (id, shops = false) => '#items/' + encodeURIComponent(id) + (shops ? '/shops' : '');
  const itemName = id => byId.get(id)?.name || data.labels?.[id] || id;
  function methodName(name,id,text) {
    if(name==='古代遗物回收' && relicOutputs.has(id)) return '遗物转换';
    if(name==='人物掉落' && matchingItemEnemies(data,id,text).length) return '敌人掉落';
    if(name==='头目掉落' && text.includes('首次击败') && data.enemyFirstRewards?.[id]?.length) return '敌人首胜奖励';
    return name;
  }
  const methods = new Map(data.items.map(item=>{
    const id=item.id, tags=new Set(Object.keys(data.guideLinks?.[id]||{}));
    const recorded=[
      ['商人出售',item.sellers?.length],['配方制作',item.recipes?.length],['远古遗迹',item.ruins?.length],
      ['牧场生产',ranchOutputs.has(id)],['遗物转换',relicOutputs.has(id)],['帕鲁远征',expeditionRewards.has(id)],
      ['敌人掉落',data.enemyDrops?.[id]?.length],['敌人首胜奖励',data.enemyFirstRewards?.[id]?.length],
      ['人物掉落',data.unitDrops?.[id]?.length],['委托与评赏报酬',data.unitRewards?.[id]?.length]
    ];
    for(const [tag,present] of recorded)if(present)tags.add(tag);
    for(const [tag,name] of item.methods||[]) tags.add(methodName(tag,id,name));
    return [id,[...tags]];
  }));
  const knownRoute = item => methods.get(item.id).length>0;
  const sourceLink = url => {
    try { const parsed = new URL(url); return parsed.protocol === 'https:' ? parsed.href : ''; }
    catch { return ''; }
  };
  let selected = 'PalSphere', isShops = false, matches = [], shown = 0;
  const sorted = data.items.slice().sort((a,b) =>
    (priority.get(a.id) ?? 999) - (priority.get(b.id) ?? 999) ||
    Number(!!a.other) - Number(!!b.other) || sortZh.compare(a.name, b.name) ||
    Number(knownRoute(b)) - Number(knownRoute(a)) || (a.rarity ?? 0) - (b.rarity ?? 0) || sortZh.compare(a.id,b.id));
  const matchesText = new Map(sorted.map(i => [i.id, [i.name, i.en, i.id, ranks[i.rarity] || ''].join(' ').normalize('NFKC').toLowerCase()]));
  const catOptions = [...new Set([...sorted.map(i => i.category),'其他游戏数据'])].sort(sortZh.compare);
  const allMethods = new Set([...methods.values()].flat());
  const methodOptions = [...new Set([...groupLabels, ...[...allMethods].sort(sortZh.compare)])].filter(k => allMethods.has(k));
  for (const value of catOptions) el('category').add(new Option(value, value));
  for (const value of methodOptions) el('method').add(new Option(value, value));
  el('summary').textContent = data.items.length.toLocaleString('zh-CN') + ' 个物品条目';

  function icon(item, large = false) {
    const url = itemIcon(item.id);
    const content = url ? '<img loading="lazy" alt="" src="' + escape(url) + '">' : escape(item.name.slice(0,1));
    return '<span class="ic-icon' + (large ? ' ic-icon-large' : '') + '" aria-hidden="true">' + content + '</span>';
  }
  root.addEventListener('error', event => {
    const img = event.target;
    if (img.tagName === 'IMG' && img.closest('.ic-icon')) img.parentElement.textContent = '◇';
  },true);
  function renderMore() {
    // The list viewport shows far fewer than 72 rows. Defer off-screen DOM;
    // scrolling or the existing More button still exposes the entire catalog.
    const batch = matches.slice(shown, shown + (shown===0 ? 24 : 72));
    list.insertAdjacentHTML('beforeend',batch.map(i => '<a class="ic-choice" href="' + escape(route(i.id)) +
      '" data-item="' + escape(i.id) + '" aria-current="' + (i.id === selected) + '">' + icon(i) + '<span class="ic-choice-name">' +
      escape(i.name) + '<small>' + escape(i.category) + (ranks[i.rarity] ? ' · ' + ranks[i.rarity] : '') + '</small></span></a>').join(''));
    shown += batch.length;
    el('more').hidden = shown >= matches.length;
  }
  function filterList() {
    const query = el('search').value.normalize('NFKC').trim().toLowerCase().split(/\s+/).filter(Boolean);
    const cat = el('category').value, method = el('method').value;
    matches = sorted.filter(i => (!cat || i.category === cat || (cat==='其他游戏数据' && i.other)) && (!method || methods.get(i.id).includes(method)) &&
      query.every(token => matchesText.get(i.id).includes(token)));
    el('count').textContent = matches.length.toLocaleString('zh-CN') + ' 个匹配物品';
    scroll.scrollTop = 0;
    list.innerHTML = matches.length ? '' : '<p class="ic-empty">没有匹配的物品。可更换关键词或清除筛选。</p>';
    shown = 0; renderMore();
  }
  function linkItem(id, caption, className = 'ic-inline') {
    return byId.has(id) ? '<a href="' + escape(route(id)) + '" class="' + className +
      '" data-item="' + escape(id) + '">' + caption + '</a>' : '<span class="' + className + '">' + caption + '</span>';
  }
  function renderRecipes(item) {
    if (!item.recipes?.length) return '<section class="ic-section"><h3>合成表</h3><p>未收录此物品的制作配方。</p></section>';
    const cards = item.recipes.map((r, idx) => {
      const ingredients = r.materials.map(([id, qty]) => linkItem(id, escape(itemName(id)) + ' <span class="ic-amount">×' + escape(qty) + '</span>')).join('');
      const requirements = [];
      if (r.tech) requirements.push('科技等级 ' + escape(r.tech));
      if (r.plan && r.plan !== 'None') requirements.push('需要 ' + linkItem(r.plan,escape(itemName(r.plan))));
      return '<div class="ic-recipe"><p>' + (item.recipes.length > 1 ? '配方 ' + (idx+1) + ' · ' : '') + '产出 ×' + escape(r.qty) +
        ' · ' + (r.stations?.length ? r.stations.map(name=>data.facilityLinks?.[name]?'<a class="ic-guide" href="#work/facilities/'+encodeURIComponent(data.facilityLinks[name])+'">'+escape(name)+'</a>':escape(name)).join('、') : '制作设施未核实') + '</p><div class="ic-ingredients">' + ingredients +
        '</div>' + (requirements.length ? '<p class="ic-small">' + requirements.join(' · ') + '</p>' : '') + '</div>';
    });
    const more = cards.length > 2 ? '<details class="ic-details-toggle"><summary>其余 ' + (cards.length-2) + ' 种配方</summary>' + cards.slice(2).join('') + '</details>' : '';
    return '<section class="ic-section"><h3>合成表</h3>' + cards.slice(0,2).join('') + more + '</section>';
  }
  const crafting = createItemCrafting({byId, detail, itemName, linkItem, escape});
  const localLink=(href,label)=>'<a class="ic-guide" href="'+escape(href)+'">'+escape(label)+'</a>';
  const enemyLink=([id,label])=>localLink('#enemies/'+encodeURIComponent(id),label);
  const externalLink=(href,label)=>'<a class="ic-guide" href="'+escape(href)+'" target="_blank" rel="noopener noreferrer">'+escape(label)+' ↗</a>';
  const mapCoord=(region,x,y,label)=>localLink('#map/coords/'+region+'/'+x+'/'+y,label||'('+x+'，'+y+')');
  const mapCategory=(key,region,label,query='')=>localLink('#map/category/'+key+'/'+region+(query?'/'+encodeURIComponent(query):''),label);
  const facility=(key,label)=>localLink('#work/facilities/'+encodeURIComponent(key),label);
  const mineralLayer={CopperOre:'ore',Coal:'coal',Sulfur:'sulfur',Quartz:'pure_quartz',RainbowCrystal:'hexolite_quartz',Chromium:'chromite',SkyIslandOre:'soralite',WorldTreeOre:'paloxite'};
  const facilityNames=Object.entries(data.facilityLinks||{}).sort((a,b)=>b[0].length-a[0].length);
  function palDropLinks(text,type,item){
    let parts=[],end=0,found=false;
    const bossSource=type==='头目掉落' || type==='狂暴帕鲁掉落';
    const drops=bossSource ? (data.enemyDrops?.[item.id] || []).concat(data.enemyFirstRewards?.[item.id] || []) : [];
    if(type==='头目掉落' && text.includes('团体战')){
      const names=Object.keys(data.enemyNames || {}).filter(name=>text.includes(name));
      if(names.length){
        const pattern=new RegExp(names.sort((a,b)=>b.length-a.length).map(escapedName).join('|'),'g');
        for(const match of text.matchAll(pattern)){
          const enemies=data.enemyNames[match[0]].filter(([id])=>id.startsWith('RAID_') && drops.some(([key])=>key===id));
          if(!enemies.length)continue;
          parts.push(escape(text.slice(end,match.index)),enemies.map(enemyLink).join(' / '));
          end=match.index+match[0].length;found=true;
        }
        if(found)return parts.join('')+escape(text.slice(end));
      }
    }
    for(const match of text.matchAll(palPattern)){
      const zh=match[0],pals=palByName.get(zh);
      parts.push(escape(text.slice(end,match.index)));
      const refs=pals.map(p=>{
        const palLink=localLink('#paldex/'+encodeURIComponent(p.key),zh+(pals.length>1?' '+p.key:''));
        const prefix=type==='头目掉落'?'BOSS_':type==='狂暴帕鲁掉落'?'PREDATOR_':'';
        const enemy=prefix && drops.find(([id])=>id.toLowerCase()===(prefix+p.key).toLowerCase());
        const entry=enemy ? localLink('#enemies/'+encodeURIComponent(enemy[0]),zh) : palLink;
        if(type!=='头目掉落')return entry;
        const sites=sourceData.bosses?.[p.key]||[];
        return [entry,...sites.map(([id,x,y])=>localLink('#map/point/'+encodeURIComponent(id),'头目 ('+x+'，'+y+')'))].join(' ');
      });
      parts.push(refs.join(' / '));end=match.index+zh.length;found=true;
    }
    return found?parts.join('')+escape(text.slice(end)):escape(text);
  }
  function acquisition(type,text,source,item,linkedRoutes){
    const region=text.includes('世界树')?'worldtree':'palpagos',refs=[];
    let body=escape(text);
    const sourceURL=sourceLink(source);
    const guides=data.guideLinks?.[item.id]||{};
    function guideLink(url,label) {
      linkedRoutes.add(url);
      return localLink(url,label);
    }
    const addMap=(key,label,query='')=>refs.push(mapCategory(key,region,label,query));
    if(['帕鲁掉落','头目掉落','狂暴帕鲁掉落','评赏奖励'].includes(type))body=palDropLinks(text,type,item);
    if(type==='人物掉落'){
      const point=sourceData.bounties?.[text];
      if(point) body=localLink('#map/point/'+encodeURIComponent(point),text);
    }
    if(type==='野外钓鱼')addMap('fishing','查看本地图钓点');
    if(type==='敌方营地'&&region==='palpagos')addMap('enemy_camp','查看本地图敌人据点');
    if(type==='地下城宝箱'&&region==='palpagos')addMap('dungeon','查看本地图地下城入口');
    if(type==='区域宝箱')addMap('chest','查看本地图宝箱');
    if(type==='油田宝箱')addMap('chest','查看油田宝箱点位','油田');
    if(type==='属性宝箱'&&region==='palpagos')addMap('chest','查看属性宝箱点位','属性宝箱');
    if(type==='属性宝箱'&&region==='worldtree'){
      addMap('chest','查看世界树全部宝箱');
    }
    const chapter=type==='属性宝箱'?'ElementalChestsLoots':itemLootChapters[type];
    if(chapter && data.lootChapters?.includes(chapter)) {
      const index='#guide/loot/'+encodeURIComponent(chapter);
      const target=(guides['探索奖励表']||[]).find(([url])=>url.startsWith(index+'/'));
      refs.push(guideLink(target?.[0]||index,'查看'+type+'奖励表'));
    }
    if(type==='技能果实树'||type==='野外采摘'&&text.includes('技能果实树')){
      if(!text.includes('世界树')||text.includes('樱岛'))refs.push(mapCategory('skill_fruit','palpagos','群岛技能果实树'));
      if(text.includes('世界树'))refs.push(mapCategory('skill_fruit','worldtree','世界树技能果实树'));
    }
    if(type==='野外帕鲁蛋'||type==='世界树拾取')addMap('egg','查看本地图帕鲁蛋点位');
    if(type==='野外采集'){
      for(const [href] of guides['野外采集']||[]) refs.push(guideLink(href,'采集方式与位置'));
      if(mineralLayer[item.id])refs.push(mapCategory(mineralLayer[item.id],item.id==='WorldTreeOre'?'worldtree':'palpagos','查看矿脉点位'));
    }
    if(type==='据点生产'){
      const id=data.facilityLinks?.[text]||itemProductionAliases[text];
      const producer=data.productionNames?.[text];
      if(producer){
        body=guideLink('#work/production/'+encodeURIComponent(producer),text);
        if(id)refs.push(facility(id,'设施详情'));
      } else if(id)body=facility(id,text);
      if(item.id==='CrudeOil'&&producer!=='OilPump02')addMap('crude_oil','查看原油点位');
    }
    if(type==='农园种植'||type==='烹饪'){
      // Longest facility names first; “厨房” alone is not a unique facility.
      let rest=text;
      for(const [name,id] of facilityNames) if(rest.includes(name)) {
        refs.push(facility(id,name));rest=rest.replaceAll(name,' ');
      }
    }
    if(type==='钓鱼池')refs.push(facility('Fishing_Pond','垂钓池'),facility('Large_Fishing_Pond','大型垂钓池'));
    if(type==='技能果树园')refs.push(facility('Skillfruit_Orchard','技能果树园'));
    if(type==='召唤战胜利奖励')refs.push(facility('Summoning_Altar','召唤的祭坛'));
    if(type==='解体产物')refs.push(facility('Pal_Disassembly_Conveyor','帕鲁解体传送带'));
    if(type==='据点配种'||type==='配种突变')refs.push(facility('Breeding_Farm','配种牧场'),localLink('#lookup','查询配种组合'));
    if(type==='困难塔主掉落'&&sourceData.treeBoss&&text.includes('泽娜拉'))refs.push(localLink('#map/point/'+sourceData.treeBoss,'定位世界树头目'));
    if((type==='古代文明遗物'||type==='遗迹与遗物')&&relicOutputs.has(item.id))refs.push(localLink('#work/relic/item/'+encodeURIComponent(item.id),'查看对应遗物'));
    if(type==='商店兑换'&&item.sellers?.length)refs.push(localLink(route(item.id,true),'商人及坐标'),localLink('#details/surgery','帕鲁手术台'));
    if(type==='任务奖励'){
      const quests=(sourceData.quests?.[text]||[]).filter(q=>q[2].includes(item.id));
      const matching=(guides['任务奖励']||[]).filter(([url,label])=>label===text || quests.some(([slug])=>url==='#guide/quests/'+encodeURIComponent(slug)));
      if(matching.length)body=matching.map(([href,label])=>guideLink(href,label)).join('、');
      for(const [,locations] of quests){
        for(const [r,x,y] of locations)refs.push(mapCoord(r,x,y,'任务坐标 ('+x+'，'+y+')'));
      }
    }
    if(['评赏奖励','巨鲸遗骨拾取','主线对话获得','任务对话获得','任务讨伐或抓捕','任务制作','任务拾取','竞技场奖励'].includes(type)){
      for(const match of text.matchAll(/(?:[（(]|约)\s*(-?\d+)\s*[,，]\s*(-?\d+)\s*[）)]/g))
        refs.push(mapCoord('palpagos',Number(match[1]),Number(match[2]),'地图 ('+match[1]+'，'+match[2]+')'));
    }
    if(sourceURL)refs.push(externalLink(sourceURL,'来源'));
    return body+(refs.length?' <span class="ic-source-jumps">'+[...new Set(refs)].join(' ')+'</span>':'');
  }
  function renderMethods(item) {
    const grouped = new Map();
    const dropEnemies = new Map(data.enemyDrops?.[item.id] || []);
    const dropPeople = new Map(data.unitDrops?.[item.id] || []);
    const linkedRoutes=new Set();
    const enemyExtras = [];
    for (const [type, text, source] of item.methods || []) {
      if (type !== '人物掉落') continue;
      for(const [href,label] of data.unitDropNames?.[item.id]?.[text]||[]) dropPeople.set(href,label);
      const linked = matchingItemEnemies(data,item.id,text);
      if (!linked.length) continue;
      for (const [id,label] of linked) dropEnemies.set(id,label);
      const point = sourceData.bounties?.[text];
      if (point) enemyExtras.push(localLink('#map/point/'+encodeURIComponent(point),'定位'+text));
      if (sourceLink(source)) enemyExtras.push(externalLink(sourceLink(source),'来源'));
    }
    if(dropEnemies.size) grouped.set('敌人掉落',Array.from(dropEnemies,enemyLink).concat([...new Set(enemyExtras)]));
    if(dropPeople.size) grouped.set('人物掉落',Array.from(dropPeople,([url,label])=>localLink(url,label)));
    if(data.unitRewards?.[item.id]?.length) grouped.set('委托与评赏报酬',data.unitRewards[item.id].map(([url,label])=>localLink(url,label)));
    if(data.enemyFirstRewards?.[item.id]?.length) grouped.set('敌人首胜奖励',data.enemyFirstRewards[item.id].map(enemyLink));
    if(ranchOutputs.has(item.id)) {
      const pals=ranchOutputs.get(item.id).map(key=>'<a class="ic-guide" href="#work/ranch/'+encodeURIComponent(key)+'">'+escape(palNames.get(key)||key)+'</a>').join('、');
      grouped.set('牧场生产',[pals+'　<a class="ic-guide" href="#work/ranch/item/'+encodeURIComponent(item.id)+'">比较各牧场等级产量</a>']);
    }
    if(relicOutputs.has(item.id)) grouped.set('遗物转换',['<a class="ic-guide" href="#work/relic/item/'+encodeURIComponent(item.id)+'">比较各档遗物的产量与概率</a>']);
    if(expeditionRewards.has(item.id)) grouped.set('帕鲁远征',expeditionRewards.get(item.id).map(s=>localLink('#work/expeditions/'+encodeURIComponent(s.id),s.name+(s.hard?'（高难）':''))));
    for (const [type, text, source] of item.methods||[]) {
      if(type==='牧场生产' && ranchOutputs.has(item.id) || type==='古代遗物回收' && relicOutputs.has(item.id)) continue;
      if(type==='帕鲁远征'&&expeditionRewards.has(item.id))continue;
      if(type==='人物掉落'&&(matchingItemEnemies(data,item.id,text).length||data.unitDropNames?.[item.id]?.[text]?.length))continue;
      const label=methodName(type,item.id,text);
      if (!grouped.has(label)) grouped.set(label,[]);
      grouped.get(label).push(acquisition(type,text,source,item,linkedRoutes));
    }
    for(const [type,links] of Object.entries(data.guideLinks?.[item.id]||{})) {
      for(const [url,label] of links) {
        if(linkedRoutes.has(url))continue;
        if(!grouped.has(type))grouped.set(type,[]);
        grouped.get(type).push(localLink(url,label));
      }
    }
    if(!grouped.size) return '';
    const rows = [...grouped].map(([type,values]) => {
      const rest = values.length > 12 ? '<details class="ic-details-toggle"><summary>另有 ' + (values.length-12) + ' 条</summary>' + values.slice(12).join('、') + '</details>' : '';
      return '<dt>' + escape(type) + '</dt><dd>' + values.slice(0,12).join('、') + rest + '</dd>';
    });
    return '<div class="ic-section"><h3>获取途径</h3><dl class="ic-methods">' + rows.join('') + '</dl></div>';
  }
  function pointAnchor(id, label, primary = false) {
    const p = data.points[id] || data.ruins[id];
    if (!p) return '';
    return '<a href="#map/point/' + encodeURIComponent(id) + '" class="' + (primary ? 'ic-action ic-primary' : 'ic-point') + '" data-point="' + escape(id) + '">' +
      escape(label || ('定位 (' + p.x + ', ' + p.y + ')')) + '</a>';
  }
  function renderSellers(item) {
    const listings = item.sellers || [];
    const known = listings.flatMap(s => data.shops[s.shop]?.points || []);
    if (!listings.length) return isShops ? '<div class="ic-section"><p>尚未收录出售此物品的商人。</p></div>' : '';
    if (!isShops) {
      const single = listings.length===1 && known.length===1;
      const listing=listings[0];
      const direct = single ? pointAnchor(known[0],'直接定位商人',true) : '';
      const info = single ? escape(data.shops[listing.shop].name) + ' · ' + escape(listing.cost.toLocaleString('zh-CN')) +
        ' ' + linkItem(listing.currency,escape(itemName(listing.currency))) + ' / ' + escape(listing.qty) + ' 件' :
        listings.length + ' 张货单' + (known.length ? ' · ' + new Set(known).size + ' 个固定位置' : '');
      return '<div class="ic-section"><h3>商人出售</h3><p>' + info + '</p><div class="ic-links">' + direct + '<a href="' +
        escape(route(item.id,true)) + '" data-item="' + escape(item.id) + '" data-shops="true" class="' +
        (single ? 'ic-action">查看货单' : 'ic-action ic-primary">查看商人') + '</a></div></div>';
    }
    return '<div class="ic-section"><h3>出售商人</h3>' + (known.length > 1 ? '<p>点击下方坐标，在世界地图定位对应商人。</p>' : '') + '</div>' +
      listings.map(s => {
        const vendor = data.shops[s.shop];
        const unitLinks=(vendor.units||[]).map(([url,label])=>localLink(url,label));
        const cost = s.cost ? escape(s.cost.toLocaleString('zh-CN')) + ' ' + linkItem(s.currency,escape(itemName(s.currency))) : '售价未收录';
        const positions = (vendor.points || []).map((id,i) => {
          const p = data.points[id], region = vendor.places?.[i];
          return pointAnchor(id,region && p ? region + ' (' + p.x + ', ' + p.y + ')' : '');
        }).join('');
        return '<article class="ic-seller"><h4>' + escape(vendor.name) + '</h4><p>' + cost + ' · 数量 ' +
          escape(s.qty) + (s.once ? ' · 限购一次' : '') + '</p>' + (positions ? '<div class="ic-links">' + positions + '</div>' : '') +
          (unitLinks.length?'<div class="ic-links">商人图鉴　'+unitLinks.join(' ')+'</div>':'')+
          (vendor.note ? '<p>' + escape(vendor.note) + '</p>' : '') + '</article>';
      }).join('');
  }
  function renderRuins(item) {
    if (!item.ruins?.length) return '';
    return '<div class="ic-section ic-ruins"><h3>远古遗迹</h3>'+item.ruins.map(id=>{
      const p=data.ruins[id];
      return '<p>遗迹奖励 · '+escape(p.rewardName)+'</p><div class="ic-links">'+
        pointAnchor(id,'定位遗迹 · '+p.nearest+'附近 ('+p.x+', '+p.y+')')+'</div>';
    }).join('')+'</div>';
  }
  function renderDetail() {
    const item = byId.get(selected);
    if (!item) {
      detail.innerHTML = '<h2>未找到这个物品</h2><p class="ic-empty">请通过左侧列表选择物品。</p>';
      return;
    }
    const back = isShops ? linkItem(item.id,'← 物品详情','ic-back') : '';
    let content = back + '<div class="ic-profile">' + icon(item,true) + '<div><h2>' + escape(item.name) +
      '</h2><p class="ic-meta">' + escape(item.category) + (ranks[item.rarity] ? ' · ' + ranks[item.rarity] : '') +
      (item.en && item.en !== item.name ? ' · ' + escape(item.en) : '') + '</p><p class="ic-meta">物品 ID · ' + escape(item.id) + '</p></div></div>' + renderSellers(item);
    if (!isShops) {
      const related=data.related?.[item.id];
      if(related?.length) content+='<div class="ic-section"><h3>关联条目</h3><div class="ic-links">'+related.map(([url,label])=>localLink(url,label)).join('')+'</div></div>';
      const vendors=data.currencies?.[item.id]||[];
      if(vendors.length){
        const links=vendors.map(([url,label])=>localLink(url,label));
        const more=links.length>6?'<details class="ic-details-toggle"><summary>其余 '+(links.length-6)+' 位商人</summary><div class="ic-links">'+links.slice(6).join(' ')+'</div></details>':'';
        content+='<section class="ic-section"><h3>接受此货币的商人</h3><div class="ic-links">'+links.slice(0,6).join(' ')+'</div>'+more+'</section>';
      }
      if(relicInputs.has(item.id)) content+='<div class="ic-section"><h3>遗物转换</h3><a class="ic-guide" href="#work/relic/'+encodeURIComponent(item.id)+'">查看这件遗物的转换产物</a></div>';
      if(data.enemySummons?.[item.id]?.length) content+='<div class="ic-section"><h3>可召唤的敌人</h3><div class="ic-links">'+data.enemySummons[item.id].map(enemyLink).join('')+'</div></div>';
      content += renderRuins(item) + renderRecipes(item) + crafting.renderPath(item) + renderMethods(item) + crafting.renderUses(item);
      if (item.note) {
        const ref=sourceLink(item.noteSource);
        content += '<div class="ic-section"><h3>物品说明</h3><p>' + escape(item.note) +
          (ref ? ' <a href="' + escape(ref) + '" target="_blank" rel="noopener noreferrer">来源 ↗</a>' : '') + '</p></div>';
      }
      if (!knownRoute(item)) {
        if (!item.note) content += '<div class="ic-section"><h3>获取途径</h3><p>' + (item.other ?
          '游戏表中保留了这个条目，尚未核实玩家能否取得。' : '已收录物品，当前资料尚未给出可核实的获取途径。') + '</p></div>';
        const namesakes = sorted.filter(other => other.id !== item.id && other.name === item.name && knownRoute(other));
        if (namesakes.length) content += '<div class="ic-section"><h3>有获取记录的同名物品</h3><div class="ic-links">' +
          namesakes.map(other => linkItem(other.id,escape(other.id) + (ranks[other.rarity] ? ' · ' + ranks[other.rarity] : ''))).join('') + '</div></div>';
      }
      if (item.slug) content += '<a class="ic-reference" href="https://paldb.cc/cn/' + escape(item.slug) +
        '" target="_blank" rel="noopener noreferrer">查看物品资料 ↗</a>';
    }
    detail.innerHTML = content;
  }
  const show = (id, subview) => {
    selected = id || (byId.has(selected) ? selected : 'PalSphere');
    isShops = subview === 'shops';
    for (const a of list.querySelectorAll('[data-item]')) a.setAttribute('aria-current', String(a.dataset.item === selected));
    renderDetail();
  };
  root.addEventListener('click',event => {
    const a=event.target.closest('a[data-item],a[data-point]');
    if (!a || event.ctrlKey || event.shiftKey || event.altKey || event.metaKey || event.button!==0) return;
    if (a.dataset.point && openMapPoint) {
      event.preventDefault();
      openMapPoint(data.points[a.dataset.point] || data.ruins[a.dataset.point]);
    } else if (a.dataset.item) {
      if (navigate) { event.preventDefault(); navigate(a.dataset.item, a.dataset.shops==='true'); }
      if (window.matchMedia('(max-width:820px)').matches) {
        requestAnimationFrame(() => { detail.focus({preventScroll:true}); detail.scrollIntoView({block:'start'}); });
      }
    }
  });
  el('search').addEventListener('input',filterList);
  el('category').addEventListener('change',filterList);
  el('method').addEventListener('change',filterList);
  el('more').addEventListener('click',renderMore);
  scroll.addEventListener('scroll',() => {
    if (shown < matches.length && scroll.scrollTop + scroll.clientHeight >= scroll.scrollHeight - 30) renderMore();
  },{passive:true});
  // The router immediately supplies the requested item after construction.
  // Build the browser once; avoid rendering the default detail before a deep link.
  filterList();
  return {show};
}

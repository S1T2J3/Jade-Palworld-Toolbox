const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');

const releaseVersion = '1.0.15';
const releaseRepository = process.env.JADE_RELEASE_REPOSITORY || 'S1T2J3/Jade-Palworld-Toolbox';
if (!/^[a-z\d][a-z\d-]{0,38}\/[a-z\d][a-z\d_.-]{0,99}$/i.test(releaseRepository)) {
  throw new Error('Invalid GitHub update repository');
}
const read = (...parts) => fs.readFileSync(path.join(__dirname, ...parts), 'utf8');
const fileUri = (mime, ...parts) => 'data:' + mime + ';base64,' + fs.readFileSync(path.join(__dirname, ...parts)).toString('base64');
const jsonBlock = (...parts) => JSON.stringify(JSON.parse(read(...parts))).replace(/</g, '\\u003c');
const desktop = process.argv[2] === '--desktop';
if (desktop && !process.argv[3]) throw new Error('Pass the desktop content directory after --desktop.');
const buildFile = desktop ? path.join(path.resolve(process.argv[3]),'index.html') :
  process.argv[2] || path.join(__dirname, '..', 'outputs', 'Jade 幻兽帕鲁工具箱.html');
const baseStyle = read('ui', 'styles.css').replace('__HERO__', fileUri('image/webp','assets','hero.webp'));
const cssModules={
  'spawn-map':'spawns','item-catalog':'items','stat-calc':'stats','capture-calc':'capture',paldex:'paldex',
  relic:'relic',ranch:'ranch',facilities:'facilities','work-efficiency':'work-efficiency',
  'combat-guide':'guide',journals:'journals',expeditions:'expeditions',
  'skin-picker':'settings','release-center':'updates',enemies:'enemies',adventure:'adventure','resource-guides':'resource-guides'
};
const originalStyles=['warehouse','trait-ui','collection','result-density','pair-grid','section-shell','world-map','spawn-map','pal-details','item-catalog','stat-calc','capture-calc','paldex','relic','ranch','facilities','work-efficiency','world-manager','breeding-gaps','partner-skills','combat-guide','journals','expeditions','handbook-layout'];
const themeStyles=require('./theme-styles.cjs');
const skinStyles=['skin-picker','skins','enemies','external-links','adventure','resource-guides','trait-ranks','release-center','global-search'];
const styles=[['base',themeStyles(baseStyle)],...originalStyles.map(name=>[name,themeStyles(read('ui',name+'.css'))]),...skinStyles.map(name=>[name,read('ui',name+'.css')])];
const style=styles.map(([name,css])=>cssModules[name]?'/*<lazy-'+name+'>*/'+css+'/*</lazy-'+name+'>*/':css).join('\n');
const core=['desktop-data','recipe-index','trait-ranks','inventory-store','surgery-book','world-store','world-manager','breeding-plans','external-footer','release-badge'];
const features={
  search:['global-search'],
  planner:['breeding-gaps','trait-ui'], runner:['trait-runner'], warehouse:['collection'], offspring:['warehouse'],
  camera:['map-camera'], map:['world-map'], spawns:['spawn-map'], details:['pal-details'],
  stats:['stat-calc'], capture:['capture-calc'], items:['item-crafting','item-catalog'], enemies:['enemies'], 'external-links':['external-links'], paldex:['paldex'], relic:['relic'], ranch:['ranch'],
  adventure:['adventure'], 'resource-guides':['resource-guides'],
  facilities:['facilities'], 'work-efficiency':['work-efficiency'], partner:['partner-skills'], guide:['combat-guide'], journals:['journals'], expeditions:['expeditions'], settings:['skin-picker'], updates:['release-center']
};
const scripts=names=>names.map(name=>read('ui',name+'.js')).join('\n;\n');
const bundledNotices=JSON.parse(read('..','announcements.json')).announcements.map(
  ({id,title,body,publishedAt,popup})=>({id,title,body,publishedAt,popup:popup===true||id==='release-v'+releaseVersion})
);
const program=scripts([...core,...(desktop?[]:Object.values(features).flat()),'app'])
  .replace('__BUNDLED_NOTICES__',()=>JSON.stringify(bundledNotices).replace(/</g,'\\u003c'))
  .replace('__BANNER_SOURCE__','https://store.steampowered.com/app/1623730/Palworld/');
const breeding=JSON.parse(read('data','breeding.json'));
const enemyBook=JSON.parse(read('enemy-data','enemies.json'));
const itemBook=JSON.parse(read('item-data','catalog.json'));
const mapBook=JSON.parse(read('map-data','map-points.json'));
const resourceBook=JSON.parse(read('resource-guides','data.json'));
const resourceMap=JSON.parse(read('resource-guides','map.json'));
const adventureBook=JSON.parse(read('adventure','data.json'));
mapBook.points.push(...resourceMap.points);
mapBook.categories.push(...resourceMap.categories);
const gathered=new Set(resourceBook.gathering.map(entry=>entry.id));
const chapterById=new Map(adventureBook.chapters.map(chapter=>[chapter.id,chapter.name]));
const mapRewards={enemy_camp:'EnemyCampLoots',dungeon:'DungeonsLoots',fishing:'FishingLoots'};
for(const category of mapBook.categories){
  if(category.id==='teafant_spring') category.itemId='WorldTreeHolyWater';
  if(gathered.has(category.itemId)) category.guide='#guide/gathering/'+encodeURIComponent(category.itemId);
  if(category.id==='crude_oil') category.guide='#work/production/OilPump';
  const chapter=mapRewards[category.id];
  if(chapterById.has(chapter)) category.lootChapter=[chapter,chapterById.get(chapter)];
}
const enemyLinks=require('./enemy-links.cjs')(enemyBook,itemBook.items,breeding.species,mapBook.points);
const humanLinks=require('./human-atlas.cjs')(enemyBook,itemBook,breeding.species,mapBook.points);
mapBook.points.push(...humanLinks.extraPoints);
mapBook.categories.push({id:'neutral_unit',name:'中立单位',group:'挑战与人物',color:'#3f7a70',default:false});
const mapIcons=JSON.parse(read('map-art','map-icons.json'));
mapIcons.defaults.neutral_unit=mapIcons.defaults.merchant;
const enemyRoutes=Object.fromEntries(enemyBook.enemies.map(unit=>[unit.id,'#enemies/'+encodeURIComponent(unit.id)]));
enemyBook.questTargets={};
adventureBook.unitRoutes={};
mapBook.questLinks={};
function addQuest(index,key,quest) {
  const rows=index[key] ||= [];
  if(!rows.some(([id])=>id===quest.id)) rows.push([quest.id,quest.name]);
}
for(const quest of adventureBook.quests) {
  for(const objective of quest.objectives) {
    const route=humanLinks.unitRoutes[objective.unit] || enemyRoutes[objective.unit];
    if(route) {
      adventureBook.unitRoutes[objective.unit]=route;
      addQuest(enemyBook.questTargets,decodeURIComponent(route.split('/').at(-1)),quest);
    }
    if(objective.point) addQuest(mapBook.questLinks,objective.point,quest);
  }
}
const guideLinks=require('./adventure-links.cjs')(adventureBook,resourceBook,itemBook.items);
const inlineJSON=obj=>JSON.stringify(obj).replace(/</g,'\\u003c');
const page = read('ui','index.html')
  .replace('<!--__WAREHOUSE__-->', () => read('ui','warehouse.html'))
  .replace('<!--__COLLECTION__-->', () => read('ui','collection.html'))
  .replace('<!--__TRAIT_UI__-->', () => read('ui','trait-ui.html'))
  .replace('__SEARCH_INDEX__', () => inlineJSON(require('./search-index.cjs')({breeding,enemyBook,itemBook,resourceBook,adventureBook,humanLinks})))
  .replace('<!--__WORLD_MANAGER__-->', () => read('ui','world-manager.html'))
  .replace('<!--__MAP_UI__-->', () => read('ui','world-map.html'))
  .replace('<!--__SPAWN_UI__-->', () => read('ui','spawn-map.html'))
  .replace('<!--__PAL_DETAILS_UI__-->', () => read('ui','pal-details.html'))
  .replace('<!--__STAT_UI__-->', () => read('ui','stat-calc.html'))
  .replace('<!--__CAPTURE_UI__-->', () => read('ui','capture-calc.html'))
  .replace('__STAT_DATA__', () => jsonBlock('stat-calc','data.json'))
  .replace('__CAPTURE_DATA__', () => {
    const capture=JSON.parse(read('capture-calc','data.json'));
    const raids=new Set(enemyBook.enemies.filter(enemy=>enemy.group==='召唤首领').map(enemy=>enemy.id));
    capture.enemiesByPal=Object.fromEntries(capture.raidEggOnly.map(key=>[key,(enemyLinks.byPal[key]||[]).filter(([id])=>raids.has(id))]));
    return inlineJSON(capture);
  })
  .replace('<!--__ITEM_UI__-->', () => read('ui','item-catalog.html'))
  .replace('<!--__PALDEX_UI__-->', () => read('ui','paldex.html'))
  .replace('<!--__ENEMIES_UI__-->', () => read('ui','enemies.html'))
  .replace('<!--__EXTERNAL_LINKS__-->', () => read('ui','external-links.html'))
  .replace('__ENEMY_DATA__', () => inlineJSON({...enemyBook,icons:JSON.parse(read('enemy-data','portraits.json')),itemNames:{...enemyLinks.itemNames,...humanLinks.itemNames}}))
  .replace('<!--__ADVENTURE_UI__-->', () => read('ui','adventure.html'))
  .replace('__ADVENTURE_DATA__', () => inlineJSON(adventureBook))
  .replace('<!--__RESOURCE_GUIDES_UI__-->', () => read('ui','resource-guides.html'))
  .replace('__RESOURCE_GUIDES_DATA__', () => inlineJSON(resourceBook))
  .replace('<!--__RELIC_UI__-->', () => read('ui','relic.html'))
  .replace('<!--__RANCH_UI__-->', () => read('ui','ranch.html'))
  .replace('<!--__FACILITIES_UI__-->', () => read('ui','facilities.html'))
  .replace('<!--__WORK_EFFICIENCY_UI__-->', () => read('ui','work-efficiency.html'))
  .replace('<!--__PARTNER_SKILLS_UI__-->', () => read('ui','partner-skills.html'))
  .replace('<!--__COMBAT_GUIDE_UI__-->', () => read('ui','combat-guide.html'))
  .replace('<!--__JOURNALS_UI__-->', () => read('ui','journals.html'))
  .replace('<!--__EXPEDITIONS_UI__-->', () => read('ui','expeditions.html'))
  .replace('<!--__SETTINGS_UI__-->', () => read('ui','skins.html'))
  .replace('<!--__RELEASE_CENTER_UI__-->', () => read('ui','release-center.html'))
  .replaceAll('__RELEASE_VERSION__', releaseVersion)
  .replaceAll('__RELEASE_REPO__', releaseRepository)
  .replace('/*__SKIN_BOOT__*/', () => read('ui','skin-boot.js'))
  .replace('__JADE_SKIN_ART__', () => fileUri('image/webp','assets','jade-selyne.webp'))
  .replace('__JADE_SYMBOL__', () => fileUri('image/webp','assets','jade-symbol.webp'))
  .replace('__PARTNER_SKILLS_DATA__', () => {
    const records=require('./partner-references.cjs')(JSON.parse(read('combat-guide','partners.json')),JSON.parse(read('pal-details','details.json')),
      JSON.parse(read('ranch-data','data.json')),JSON.parse(read('item-data','catalog.json')).items);
    return JSON.stringify(records).replace(/</g,'\\u003c');
  })
  .replace('__COMBAT_GUIDE_DATA__', () => jsonBlock('combat-guide','combat.json'))
  .replace('__JOURNALS_DATA__', () => jsonBlock('journals','data.json'))
  .replace('__EXPEDITION_DATA__', () => jsonBlock('expeditions','data.json'))
  .replace('__WORK_EFFICIENCY_DATA__', () => jsonBlock('work-efficiency','data.json'))
  .replace('__FACILITIES_DATA__', () => {
    const facilities=JSON.parse(read('facilities','data.json'));
    for(const facility of facilities.facilities) {
      const links=resourceBook.production.filter(row=>row.facility===facility.id).map(row=>[row.id,row.name]);
      if(links.length) facility.production=links;
    }
    return inlineJSON(facilities);
  })
  .replace('__PALDEX_DATA__', () => {
    const profiles=JSON.parse(read('paldex-data','data.json'));
    for(const [key,links] of Object.entries(enemyLinks.byPal)) profiles[key].enemies=links;
    for(const [key,links] of Object.entries(humanLinks.palSellers)) profiles[key].sellers=links;
    return inlineJSON(profiles);
  })
  .replace('__PALDEX_INDEX__', () => jsonBlock('paldex-data','index.json'))
  .replace('__RELIC_DATA__', () => jsonBlock('relic-data','data.json'))
  .replace('__RANCH_DATA__', () => jsonBlock('ranch-data','data.json'))
  .replace('__PAL_DETAILS_DATA__', () => jsonBlock('pal-details','details.json'))
  .replace('__ITEM_DATA__', () => {
    const catalog=itemBook;
    for(const field of ['enemyDrops','enemyNames','enemyFirstRewards','enemySummons']) catalog[field]=enemyLinks[field];
    catalog.guideLinks=guideLinks;
    catalog.productionNames=Object.fromEntries(resourceBook.production.flatMap(row=>
      [row.name,...row.aliases].map(name=>[name,row.id])));
    catalog.lootChapters=adventureBook.chapters.map(chapter=>chapter.id);
    catalog.unitDrops=humanLinks.dropsByItem;
    catalog.unitRewards=humanLinks.requestRewards;
    catalog.unitDropNames=humanLinks.namesByItem;
    for(const [id,links] of Object.entries(humanLinks.unitsByShop)) catalog.shops[id].units=links;
    const names=JSON.parse(read('facilities','data.json')).facilities.map(f=>[f.name,f.id]);
    catalog.facilityLinks=Object.fromEntries(names);
    catalog.related=require('./related-items.cjs')(JSON.parse(read('pal-details','details.json')),JSON.parse(read('combat-guide','partners.json')),JSON.parse(read('combat-guide','combat.json')));
    function relate(id,route,label) {
      const rows=catalog.related[id] ||= [];
      if(!rows.some(([href])=>href===route))rows.push([route,label]);
    }
    for(const quest of adventureBook.quests) {
      for(const goal of quest.objectives) if(goal.item) relate(goal.item,'#guide/quests/'+encodeURIComponent(quest.id),'任务 · '+quest.name);
    }
    for(const record of resourceBook.gathering) {
      for(const [tool] of record.tools||[]) relate(tool,'#guide/gathering/'+encodeURIComponent(record.id),'采集 · '+record.name);
    }
    catalog.currencies={};
    for(const item of catalog.items) for(const sale of item.sellers||[]) {
      const vendors=catalog.currencies[sale.currency] ||= [];
      for(const [route,label] of catalog.shops[sale.shop].units||[]) {
        if(!vendors.some(([url])=>url===route))vendors.push([route,label]);
      }
    }
    catalog.acquisitionLinks=JSON.parse(read('expeditions','item-links.json'));
    return JSON.stringify(catalog).replace(/</g,'\\u003c');
  })
  .replace('__ITEM_ICONS__', () => jsonBlock('item-art','icons.json'))
  .replace('__SPAWN_DATA__', () => jsonBlock('spawn-data','spawn-data.json'))
  .replace('__MAP_DATA__', () => inlineJSON({...mapBook,enemyLinks:enemyLinks.byPoint,unitLinks:humanLinks.byPoint}))
  .replace('__MAP_IMAGES__', () => JSON.stringify({
    palpagos:fileUri('image/webp','map-art','palpagos.webp'),
    worldtree:fileUri('image/webp','map-art','worldtree.webp'),
    minerals:{...JSON.parse(read('map-art','mineral-icons.json')),...resourceMap.icons},
    mapIcons
  }))
  .replace('/*__TRAIT_SOLVER__*/', () => read('ui','trait-solver.js'))
  .replace('/*__WAREHOUSE_SOLVER__*/', () => read('ui','warehouse-solver.js'))
  .replace('/*__OFFSPRING_SOLVER__*/', () => read('ui','offspring-solver.js'))
  .replace('__TRAITS__', () => JSON.stringify({...JSON.parse(read('data','traits.json')),terminalNames:JSON.parse(read('ui','terminal-names.json'))}).replace(/</g,'\\u003c'))
  .replace('__FAVICON__', () => fileUri('image/x-icon','desktop','jade-favicon.ico'))
  .replace('/*__CSS__*/', () => style)
  .replace('__DATABASE__', () => JSON.stringify(require('./compact-pairs.cjs')(breeding)).replace(/</g,'\\u003c'))
  .replace('__PORTRAITS__', () => jsonBlock('assets','portraits.json'))
  .replace('/*__APP__*/', () => program);
if (/__[A-Z_]{2,}__/.test(page)) throw new Error('Unresolved build placeholder');
function makeDesktopPage(html) {
  const target=path.dirname(buildFile), assets=path.join(target,'assets'), tables=path.join(target,'tables'), modules=path.join(target,'modules');
  for(const folder of [assets,tables,modules]) fs.mkdirSync(folder,{recursive:true});
  const digest=data=>crypto.createHash('sha256').update(data).digest('hex').slice(0,32);
  const images=new Map();
  const splitImages=part=>part.replace(/data:image\/(webp|x-icon);base64,([A-Za-z0-9+/=]+)/g,(uri,kind,base64)=>{
    if(images.has(uri)) return images.get(uri);
    const bytes=Buffer.from(base64,'base64');
    const name=digest(bytes)+(kind==='webp'?'.webp':'.ico');
    fs.writeFileSync(path.join(assets,name),bytes);
    const src='assets/'+name;
    images.set(uri,src);return src;
  });
  const {splitDesktopImages}=require('./desktop/portrait-assets.cjs');
  let light=splitDesktopImages(html,splitImages,JSON.parse(read('data','breeding.json')));
  // Use local classic scripts: file:// reads work in the packaged content
  // sandbox without a localhost server or relaxed cross-origin permissions.
  const optional=new Set(['search-index-data','map-data','spawn-data','item-catalog-data','enemy-data','adventure-data','resource-guides-data','paldex-data','paldex-index-data','pal-details-data','stat-data','capture-data','relic-data','ranch-data','item-icons','map-images','facilities-data','work-efficiency-data','partner-skills-data','combat-guide-data','journals-data','expedition-data']);
  light=light.replace(/<script id="([^"]+)" type="application\/json">([\s\S]*?)<\/script>/g,(tag,id,json)=>{
    if(!optional.has(id)) return tag;
    optional.delete(id);
    const source='document.getElementById('+JSON.stringify(id)+').textContent='+JSON.stringify(json)+';\n';
    const name=id+'-'+digest(source)+'.js';
    fs.writeFileSync(path.join(tables,name),source);
    return '<script id="'+id+'" type="application/json" data-local-table="tables/'+name+'"></script>';
  });
  if(optional.size) throw new Error('Missing tables: '+[...optional].join(', '));
  function featureTag(id,source) {
    const name=id+'-'+digest(source)+'.js';
    fs.writeFileSync(path.join(modules,name),source);
    return '<script id="'+id+'" type="application/x-palatlas-code" data-local-module="modules/'+name+'"></script>';
  }
  // The same solver must also run in a Blob worker on file://. Keep one
  // copy on disk; expose the factory's source to the existing worker runner
  // only once the corresponding calculator is opened.
  const solverNames={'trait-solver-code':'TraitSolver','warehouse-solver-code':'WarehouseSolver','offspring-solver-code':'OffspringSolver'};
  light=light.replace(/<script id="([^"]+-solver-code)">([\s\S]*?)<\/script>/g,(_,id,source)=>{
    const symbol=solverNames[id],factory='make'+symbol;
    if(!symbol) throw new Error('Unknown solver: '+id);
    const moduleSource=`function ${factory}() {
${source}
return ${symbol};
}
const ${symbol} = ${factory}();
document.getElementById(${JSON.stringify(id)}).textContent = "const ${symbol} = (" + ${factory}.toString() + ")();";
`;
    return featureTag(id,moduleSource);
  });
  const setup=new Map(Object.keys(features).map(name=>[name,[]]));
  const append=(name,source)=>setup.get(name).push(source);
  light=light.replace(/\/\*<lazy-([\w-]+)>\*\/([\s\S]*?)\/\*<\/lazy-\1>\*\//g,(_,name,css)=>{
    const id='style-'+name;
    append(cssModules[name],'document.getElementById('+JSON.stringify(id)+').textContent='+JSON.stringify(css)+';');
    return '</style><style id="'+id+'"></style><style>';
  });
  light=require('./desktop/deferred-panels.cjs').splitPanels(light,append);
  const tags=Object.entries(features).map(([name,files])=>featureTag('code-'+name,[...setup.get(name),scripts(files)].join('\n'))).join('\n');
  light=light.replace('<script id="trait-solver-code"',tags+'\n<script id="trait-solver-code"');
  return light.replace('<head>','<head>\n  <meta name="pal-desktop" content="standalone">');
}
fs.writeFileSync(buildFile, desktop ? makeDesktopPage(page) : require('./portable-page.cjs')(page));
console.log(buildFile + ' (' + fs.statSync(buildFile).size + ' bytes)');

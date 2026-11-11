// Standalone HTML already contains these resources. The desktop package
// uses classic scripts so its file:// sandbox also works entirely offline.
function mountAtlasPanel(id,html) {
  const panel=document.getElementById(id);
  panel.innerHTML=html;
  panel.dispatchEvent(new Event('atlas-panel-mounted',{bubbles:true}));
}
function createAtlasDataLoader() {
  const blocks=new Map([...document.querySelectorAll('script[data-local-table],script[data-local-module],script[data-embedded-table]')].map(node=>[node.id,node]));
  if(!blocks.size) return {wait:()=>false,load:()=>Promise.resolve(),startBackground:()=>{}};
  const planner=['code-planner','code-runner','trait-solver-code','pal-details-data'];
  const units=['code-enemies','enemy-data','item-icons'];
  const resources=['code-resource-guides','resource-guides-data','item-icons'];
  const adventure=['code-adventure','adventure-data','item-icons'];
  const routes={
    about:['map-data','spawn-data','item-catalog-data','pal-details-data'],
    editor:planner, planner:[...planner,'spawn-data'], warehouse:['code-warehouse'], settings:['code-settings'], updates:['code-updates'],
    offspring:['code-offspring','code-runner','trait-solver-code','warehouse-solver-code','offspring-solver-code'],
    map:['code-camera','code-map','map-data','map-images'], spawns:['code-camera','code-spawns','map-data','map-images','spawn-data'],
    details:['code-details','pal-details-data','spawn-data','item-icons'], stats:['code-stats','stat-data'], capture:['code-capture','capture-data'],
    paldex:['code-paldex','paldex-data','paldex-index-data','pal-details-data','ranch-data','item-icons'],
    items:['code-items','item-catalog-data','relic-data','ranch-data','expedition-data','item-icons'],
    enemies:units, merchants:units, neutrals:units,
    gathering:resources, production:resources, loot:adventure, quests:adventure,
    ranch:['code-ranch','item-catalog-data','ranch-data','item-icons'], relic:['code-relic','item-catalog-data','relic-data','item-icons'],
    suitability:['code-details','pal-details-data','paldex-index-data'], facilities:['code-facilities','facilities-data','item-catalog-data','item-icons'],
    efficiency:['code-work-efficiency','work-efficiency-data','paldex-index-data'],
    partner:['code-partner','partner-skills-data'], elements:['code-guide','combat-guide-data'], status:['code-guide','combat-guide-data'],
    journals:['code-journals','journals-data'], expeditions:['code-expeditions','expedition-data','item-catalog-data','item-icons']
  };
  const loading=new Map(), status=document.getElementById('atlas-load-status');
  const message=status.querySelector('span'), retryButton=status.querySelector('button');
  const requests={view:0,about:0,editor:0};
  let timer, retry=null, foreground=0;
  retryButton.addEventListener('click',()=>retry?.());
  function load(id) {
    if(!blocks.has(id)) return Promise.resolve();
    if(loading.has(id)) return loading.get(id);
    const block=blocks.get(id);
    const task=new Promise((resolve,reject)=>{
      if(block.hasAttribute('data-embedded-table')) {
        readEmbeddedAtlasTable(block).then(()=>{blocks.delete(id);resolve();},reject);
        return;
      }
      const script=document.createElement('script');
      script.src=block.dataset.localTable || block.dataset.localModule;
      script.onload=()=>{
        script.remove();
        if(!block.dataset.localTable || block.textContent.length) {blocks.delete(id);resolve();}
        else reject(new Error(id));
      };
      script.onerror=()=>{script.remove();reject(new Error(id));};
      document.head.append(script);
    }).finally(()=>loading.delete(id));
    loading.set(id,task);return task;
  }
  // Prepare unopened pages after the first usable frame, without initializing
  // their maps/calculators or performing any native or online actions.
  let backgroundStarted=false, backgroundRunning=false, backgroundTimer=0, idleHandle=0;
  let lastInput=0, backgroundQueue=[], warmedImage=null;
  const constrained=(navigator.hardwareConcurrency||2)<=4 || (navigator.deviceMemory&&navigator.deviceMemory<4);
  const quietTime=constrained?1200:700, batchGap=constrained?350:100;
  const now=()=>performance.now();
  function scheduleBackground(delay=batchGap) {
    if(!backgroundStarted || backgroundRunning || backgroundTimer || idleHandle || !backgroundQueue.length || document.hidden) return;
    backgroundTimer=setTimeout(()=>{
      backgroundTimer=0;
      if(document.hidden)return;
      if(foreground || now()-lastInput<quietTime || navigator.scheduling?.isInputPending?.()) {
        scheduleBackground(quietTime);return;
      }
      if(window.requestIdleCallback) idleHandle=window.requestIdleCallback(deadline=>{
        idleHandle=0;
        if(deadline.timeRemaining()<5){scheduleBackground();return;}
        runBackground();
      });
      else runBackground();
    },delay);
  }
  async function warmDefaultMap() {
    // At most one large map texture is warmed. On smaller machines only its
    // compressed file is read; forcing two 4096-square bitmaps wastes memory.
    const table=document.getElementById('map-images');
    if(!table?.textContent || /^#(?:map|spawns)(?:\/|$)/.test(location.hash))return;
    const src=JSON.parse(table.textContent).palpagos;
    if(!src)return;
    const image=new Image();image.decoding='async';
    warmedImage=image;
    await new Promise((resolve,reject)=>{image.onload=resolve;image.onerror=reject;image.src=src;});
    if(!constrained && !document.hidden)await image.decode?.();
  }
  async function runBackground() {
    if(document.hidden || foreground || now()-lastInput<quietTime || navigator.scheduling?.isInputPending?.()) {
      scheduleBackground(quietTime);return;
    }
    let id;
    while(backgroundQueue.length) {
      id=backgroundQueue.shift();
      if(id==='warm-default-map' || blocks.has(id))break;
      id=null;
    }
    if(!id)return;
    backgroundRunning=true;
    try {if(id==='warm-default-map')await warmDefaultMap();else await load(id);}
    catch { /* A failed background read stays available for the normal retry UI. */ }
    finally {backgroundRunning=false;scheduleBackground();}
  }
  function startBackground() {
    if(backgroundStarted)return;
    backgroundStarted=true;lastInput=now();
    const preferred=[...routes.map,'warm-default-map',...planner,...routes.warehouse,
      'code-search','search-index-data',...routes.items,...routes.paldex,...routes.spawns];
    backgroundQueue=[...new Set([...preferred,...blocks.keys()])].filter(id=>id!=='code-updates');
    const activity=()=>{lastInput=now();};
    for(const type of ['pointerdown','pointermove','keydown','wheel','input','touchstart'])
      document.addEventListener(type,activity,{capture:true,passive:true});
    document.addEventListener('visibilitychange',()=>{
      if(document.hidden) {
        clearTimeout(backgroundTimer);backgroundTimer=0;
        if(idleHandle)window.cancelIdleCallback?.(idleHandle);idleHandle=0;
        warmedImage=null;
      } else {lastInput=now();scheduleBackground(quietTime);}
    });
    const afterReady=()=>requestAnimationFrame(()=>requestAnimationFrame(()=>scheduleBackground(1200)));
    if(document.readyState==='loading')document.addEventListener('DOMContentLoaded',afterReady,{once:true});
    else afterReady();
  }
  return {load,startBackground,wait(route,again,channel='view') {
    const [page,subpage]=route.split('/');
    const request=++requests[channel],hash=location.hash;
    const current=()=>request===requests[channel] && hash===location.hash;
    clearTimeout(timer);status.hidden=true;retry=null;
    const needed=routes[page]||[];
    const extra=page==='details' && (subpage==='stats'||subpage==='capture') ? routes[subpage] : [];
    const missing=[...needed,...extra].filter(id=>blocks.has(id));
    if(!missing.length) return false;
    foreground++;
    retryButton.hidden=true;message.textContent='正在读取本地资料…';
    timer=setTimeout(()=>{if(current())status.hidden=false;},140);
    Promise.all(missing.map(load)).then(()=>{
      if(current()) again();
    }).catch(()=>{
      if(!current()) return;
      clearTimeout(timer);message.textContent='未能读取本地资料。请重试，或重新打开手册。';
      retry=again;retryButton.hidden=false;status.hidden=false;
    }).finally(()=>{foreground--;scheduleBackground();});
    return true;
  }};
}

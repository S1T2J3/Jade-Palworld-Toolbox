function initOffspringPage({species,pairs,readPairs=()=>pairs,inventory,avatar,dex,icon,escape,normalize,matchScore,resultsPerPage,spawnLink}) {
  const byId=id=>document.getElementById(id);
  const resultField=byId('wh-result-search'), limitSelect=byId('wh-limit');
  const routeDialog=byId('wh-route-dialog');
  const calculate=createOfflineRunner(request=>OffspringSolver.solve(request),
    ['trait-solver-code','warehouse-solver-code','offspring-solver-code']);
  let stamp=null, loading=false, failed=false, uncertain=0, page=1, target=null;
  let stock=new Map(), owned=new Set(), reachable=[], listed=[], limit='1';
  const label=i=>escape(species[i].zh);
  const gender=sex=>sex==='M'?'雄':'雌';

  function setupLimits() {
    const max=Math.max(5,...reachable.map(item=>item.depth),limit==='all'?0:Number(limit));
    limitSelect.replaceChildren();
    for (let i=1;i<=max;i++) limitSelect.add(new Option(i+' 步以内',String(i)));
    limitSelect.add(new Option('不限步数','all'));
    limitSelect.value=limit;
  }
  function showRoute(id) {
    const result=reachable.find(item=>item.id===id);
    if (!result) return;
    target=id;
    const found=new Map();
    const operations=result.steps.map(({child,parents,depth})=>{
      const [a,b]=parents;
      const pieces=parents.map(p=>{
        const member=stock.get(p.stockId);
        if (member) found.set(member.id,member);
        const name=member?.name ? ' · '+escape(member.name) : '';
        const sex=gender(p.sex)+(p.unknown?'（待确认）':'');
        return `<span>${spawnLink(p.species)}${name}<span class="wh-sex">${sex}</span></span>`;
      });
      return `<li data-wh-child="${child}" data-wh-a="${a.species}" data-wh-b="${b.species}" data-wh-depth="${depth}"><span class="wh-route-generation">第 ${depth} 步</span><div class="wh-equation">${pieces[0]} × ${pieces[1]} ${icon('arrow')}<b>${spawnLink(child)}</b></div></li>`;
    }).join('');
    const founders=Array.from(found.values(),r=>label(r.species)+(r.name?' · '+escape(r.name):'')).join('、');
    byId('wh-route-body').innerHTML=`<div class="wh-route-target">${avatar(species[id])}<div><h3>${spawnLink(id)}</h3><p>${result.minimumCertain?'最少':'此路线'} ${result.depth} 代 · 本路线 ${result.steps.length} 组配对</p></div></div><p class="wh-route-founders"><b>用到的仓库个体　</b>${founders}</p><ol class="wh-route-steps">${operations}</ol><p class="wh-route-foot">${result.conditional?'请先核对标注「待确认」的个体性别。':''}子代用于后续配对时，请留下路线要求的性别。</p>`;
    if (!routeDialog.open) routeDialog.showModal();
  }
  function emptyState(heading, body, reset=false) {
    return `<div class="wh-empty"><span class="wh-empty-icon">${icon('egg')}</span><strong>${heading}</strong><p>${body}</p>${reset ? '<button type="button" id="wh-clear-filter">清除搜索</button>' : ''}</div>`;
  }
  function renderResults() {
    const q = normalize(resultField.value);
    const threshold = limit === 'all' ? Infinity : Number(limit);
    const onlyNew = byId('wh-new-only').checked;
    const possible = reachable.filter(item => !onlyNew || !owned.has(item.id));
    listed = possible.filter(item => item.depth <= threshold && matchScore(item.id,q) > 0);
    byId('wh-count').textContent = listed.length;
    byId('wh-table-header').hidden = !listed.length;
    const pages = byId('wh-pages');
    pages.hidden = !listed.length;
    const resultContainer = byId('wh-results');
    if (!listed.length) {
      if (loading) resultContainer.innerHTML=emptyState('正在计算可配子代','按仓库个体的性别检查每一代配对。');
      else if (failed) resultContainer.innerHTML=emptyState('计算未完成','<button type="button" id="of-retry">重新计算</button>');
      else if (!owned.size) resultContainer.innerHTML = emptyState('仓库里还没有帕鲁','<a href="#warehouse">前往配种仓库添加个体</a>');
      else if (!reachable.length) resultContainer.innerHTML = emptyState('当前仓库没有可用配对','需要两只能够配种的帕鲁组成一雌一雄。');
      else if (!possible.length) resultContainer.innerHTML = emptyState('当前仓库配不出新物种','可以加入其他亲代，或取消「仅看未拥有」查看能繁育的已有物种。');
      else if (q) resultContainer.innerHTML = emptyState('没有找到符合条件的子代','可以更换目标子代，或放宽步数上限。',true);
      else resultContainer.innerHTML = emptyState('此步数内没有新物种','提高步数上限以查看更远的子代。');
      return;
    }
    const pageSize = resultsPerPage(listed.length);
    const lastPage = Math.ceil(listed.length/pageSize);
    page = Math.min(page,lastPage);
    const start=(page-1)*pageSize, stop=Math.min(start+pageSize,listed.length);
    resultContainer.innerHTML = listed.slice(start,stop).map(item => {
      const i=item.id, pal=species[i], depth=item.depth;
      const [a,b]=item.steps[item.steps.length-1].parents.map(p=>p.species);
      const condition=item.conditional?'<small class="of-condition">需确认性别</small>':'';
      return `<button class="wh-result" type="button" data-wh-route="${i}" data-wh-depth="${depth}" aria-label="${label(a)}与${label(b)}配出${label(i)}，${depth}步${item.conditional?'，需要确认亲代性别':''}，查看路线" aria-haspopup="dialog">
        <span class="wh-source"><span class="wh-parent-preview"><span>${label(a)}</span> <span class="wh-cross">×</span> <span>${label(b)}</span></span>${condition}</span>
        ${icon('arrow')}
        <span class="member">${avatar(pal)}<span class="wh-child-text"><strong>${label(i)}</strong><span class="wh-child-meta"><span class="member-id">${escape(dex(pal))}</span><span class="wh-step-badge">${depth} 步</span></span></span></span>
      </button>`;
    }).join('');
    const arrows = [['first','第一页','first',page===1],['prev','上一页','left',page===1],['next','下一页','right',page===lastPage],['last','最后一页','last',page===lastPage]];
    const buttons=arrows.map(([action,title,glyph,disabled]) => `<button type="button" data-wh-page="${action}" aria-label="${title}" title="${title}" ${disabled?'disabled':''}>${icon(glyph)}</button>`);
    const navigation=lastPage>1 ? `<div class="page-controls">${buttons[0]}${buttons[1]}<span class="page-indicator">${page} / ${lastPage}</span>${buttons[2]}${buttons[3]}</div>` : '';
    pages.innerHTML = `<span>显示 ${start+1}–${stop} 项，共 ${listed.length} 项</span>${navigation}`;
  }
  function filterChanged() {page=1; renderResults();}
  byId('wh-new-only').addEventListener('change',filterChanged);
  resultField.addEventListener('input',event => {if (!event.isComposing) filterChanged();});
  resultField.addEventListener('compositionend',filterChanged);
  byId('wh-results').addEventListener('click',event => {
    if (event.target.closest('#of-retry')) {stamp=null;refresh();return;}
    const button=event.target.closest('[data-wh-route]');
    if (button) {showRoute(Number(button.dataset.whRoute));return;}
    if (event.target.closest('#wh-clear-filter')) {resultField.value='';filterChanged();resultField.focus();}
  });
  byId('wh-pages').addEventListener('click',event => {
    const button=event.target.closest('[data-wh-page]');
    if (!button || button.disabled) return;
    const action=button.dataset.whPage;
    page={first:1,prev:page-1,next:page+1,last:Math.ceil(listed.length/resultsPerPage(listed.length))}[action];
    renderResults();
    const results=byId('wh-results');
    results.focus({preventScroll:true});
    const header=byId('wh-table-header');
    if (header.getBoundingClientRect().top<0) header.scrollIntoView({block:'start'});
  });
  byId('wh-route-close').addEventListener('click',() => routeDialog.close());
  routeDialog.addEventListener('click',event => {
    if (event.target !== routeDialog) return;
    const r=routeDialog.getBoundingClientRect();
    if (event.clientX < r.left || event.clientX > r.right || event.clientY < r.top || event.clientY > r.bottom) routeDialog.close();
  });
  routeDialog.addEventListener('close',() => {target=null;});
  let revision=0;
  limitSelect.addEventListener('change',()=>{
    limit=limitSelect.value; page=1;
    inventory.setLimit(limit);
    renderResults();
  });
  async function refresh() {
    if (byId('offspring-view').hidden) return;
    const rows=inventory.list().filter(p=>p.species>=0);
    stock=new Map(rows.map(p=>[p.id,p]));
    owned=new Set(rows.map(p=>p.species));
    const unknown=rows.filter(p=>p.sex==='unknown').length;
    byId('wh-members').textContent=`仓库共 ${rows.length} 只、${owned.size} 种。每一步为一代。` +
      (unknown ? ` ${unknown} 只性别待确认，相关路线已标注。` : '');
    limit=inventory.getLimit();
    const signature=JSON.stringify(rows.map(p=>[p.id,p.species,p.sex]).sort((a,b)=>a[0].localeCompare(b[0])));
    if (signature===stamp) {
      setupLimits();renderResults();
      if (routeDialog.open && target!==null) showRoute(target);
      return;
    }
    stamp=signature;
    const run=++revision;
    if (routeDialog.open) routeDialog.close();
    reachable=[];page=1;failed=false;loading=rows.length>=2;uncertain=0;
    byId('wh-uncertain').hidden=true;
    setupLimits();renderResults();
    if (!loading) return;
    try {
      const result=await calculate({pairs:readPairs(),speciesCount:species.length,stock:rows.map(({id,species,sex})=>({id,species,sex,traits:[]}))});
      if (run!==revision) return;
      reachable=result.items;uncertain=result.unresolved;
    } catch {
      if (run!==revision) return;
      failed=true;
    }
    loading=false;
    byId('wh-uncertain').hidden=!uncertain;
    byId('wh-uncertain').textContent=`另有 ${uncertain} 种子代的路线受未登记性别影响，补录性别后可再计算。`;
    setupLimits();renderResults();
  }
  return {refresh,resetPage:filterChanged,closeDialog:()=>{if (routeDialog.open) routeDialog.close();}};
}

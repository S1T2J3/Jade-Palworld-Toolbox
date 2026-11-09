function initPalCollection({species,traitData,avatar,dex,icon,escape,normalize,matchScore,inventory,addIndividual,editIndividual,spawnLink}) {
  const $ = id => document.getElementById(id);
  const grid = $('cl-cards');
  const picker = $('cl-picker');
  const chooser = $('cl-add-search');
  const choices = $('cl-choices');
  const search = $('cl-search');
  const lookup = new Map(traitData.traits.map(trait => [trait.key,trait]));
  const speciesIds = species.map((_,i) => i);
  const pageSize = 24;
  const sexName = value => ({M:'雄性',F:'雌性'})[value] || '性别未录';
  const gameSpeciesName=record=>{
    const key=record.terminalSpeciesKey?.toLowerCase();
    return key && Object.hasOwn(traitData.terminalNames||{},key)?traitData.terminalNames[key]:species[record.species]?.zh||record.speciesKey;
  };
  const symbols = {
    edit:'<path d="m4 15-.8 5.7L9 20l11-11-5-5zM13.8 5.2l5 5"/>',
    trash:'<path d="M4 6h16M9 6V3.7h6V6M6 6l1 14h10l1-14M10 9.5v7m4-7v7"/>',
    plus:'<path d="M12 5v14M5 12h14"/>'
  };
  const symbol = name => `<svg aria-hidden="true" class="cl-symbol" viewBox="0 0 24 24">${symbols[name]}</svg>`;
  let page=1, visible=[], shownSpecies=[], archiveMode=false, editing=null;
  function traitNames(record) {
    return record.traits.map(k=>lookup.get(k)||{zh:k,en:k,rank:0});
  }
  function matchRecord(record,q) {
    if (!q || (record.species>=0 ? matchScore(record.species,q) : normalize(record.speciesKey).includes(q))) return true;
    if (normalize(record.name || '').includes(q) || (record.terminalSpeciesKey && normalize(gameSpeciesName(record)).includes(q))) return true;
    return traitNames(record).some(trait=>normalize(trait.zh).includes(q) || normalize(trait.en).includes(q));
  }
  function renderCard(record) {
    const pal=species[record.species], id=escape(record.id),speciesName=gameSpeciesName(record);
    const traits=traitNames(record);
    const nickname=record.name && record.name!==speciesName ? record.name : '';
    const itemDescription=escape(speciesName+(nickname ? ' '+nickname : ''));
    const tokens=traits.map(t=>{
      const name=escape(t.zh);
      const value=t.key?`<a href="#details/passive/trait/${encodeURIComponent(t.key)}">${passiveRankIcon(t.rank)}${name}</a>`:name;
      return `<li class="cl-trait" data-rank="${Number(t.rank)}" ${t.rank<0?'data-negative':''}>${value}</li>`;
    }).join('');
    const list=tokens ? `<ul class="cl-traits">${tokens}</ul>` : '<p class="cl-no-traits">尚未登记词条</p>';
    return `<article class="cl-card" data-cl-id="${id}" data-cl-species-index="${record.species}" data-element="${escape(pal?.types[0]||'')}" aria-label="${itemDescription}">
      <div class="cl-card-head">${pal?avatar(pal):''}<div class="cl-identity"><div class="cl-identifiers"><span class="cl-dex">${pal?escape(dex(pal)):'图鉴待收录'}</span><span class="cl-sex" data-sex="${escape(record.sex)}">${sexName(record.sex)}</span></div><h3>${pal?spawnLink(record.species,speciesName):escape(speciesName)}</h3>${nickname ? '<p class="cl-nick">'+escape(nickname)+'</p>' : ''}</div></div>
      <div class="cl-card-body"><h4>被动词条</h4>${list}</div>
      <div class="cl-card-actions">${pal?`<button type="button" data-cl-edit="${id}" aria-label="编辑${itemDescription}">${symbol('edit')}编辑</button>
      <button type="button" data-cl-more="${record.species}" aria-label="再添加一只${escape(speciesName)}" title="再添加一只${escape(speciesName)}">${symbol('plus')}<span class="cl-action-word">添加同种</span></button>`:''}
      <button type="button" data-cl-remove="${id}" aria-label="移除${itemDescription}" title="移除${itemDescription}">${symbol('trash')}</button></div></article>`;
  }
  function renderEmpty(noMatch) {
    const art=noMatch ? '' : `<div class="cl-empty-art" aria-hidden="true">${[1,0,2].filter(i=>species[i]).map(i=>avatar(species[i])).join('')}</div>`;
    return `<div class="cl-empty">${art}<h3>${noMatch?'没有匹配的仓库成员':'仓库里还没有帕鲁'}</h3><p>${noMatch?'可以改用物种名、个体名或被动词条查找。':'添加第一只帕鲁，即可登记它的性别和被动词条。'}</p><button type="button" data-cl-empty="${noMatch?'clear':'add'}">${noMatch?'清除搜索':'添加第一只帕鲁'}</button></div>`;
  }
  function renderPages() {
    const container=$('cl-pages'), last=Math.ceil(visible.length/pageSize);
    container.hidden=last<=1;
    if (last<=1) { container.replaceChildren(); return; }
    const pages=[['first','第一页','first',page===1],['prev','上一页','left',page===1],['next','下一页','right',page===last],['last','最后一页','last',page===last]];
    const buttons=pages.map(([action,label,glyph,disabled])=>`<button type="button" data-cl-page="${action}" title="${label}" aria-label="${label}" ${disabled?'disabled':''}>${icon(glyph)}</button>`);
    const start=(page-1)*pageSize+1;
    container.innerHTML=`<span>显示 ${start}–${Math.min(start+pageSize-1,visible.length)} 只</span><div class="cl-page-controls">${buttons[0]}${buttons[1]}<span>${page} / ${last}</span>${buttons[2]}${buttons[3]}</div>`;
  }
  function refresh() {
    const rows=inventory.list();
    const q=normalize(search.value);
    visible=rows.filter(row=>matchRecord(row,q)).sort((a,b)=>a.species-b.species);
    page=Math.max(1,Math.min(page,Math.ceil(visible.length/pageSize)));
    const totalKinds=new Set(rows.map(record=>record.speciesKey)).size;
    $('cl-count').textContent=(q ? `找到 ${visible.length} 只 / 共 ` : '')+`${rows.length} 只 · ${totalKinds} 种`;
    $('cl-save').textContent=inventory.persistent===false?'仅本次打开有效':'保存在本机';
    const archivedCount=inventory.archived?.().length || 0;
    const archiveLink=$('cl-archive-open');
    archiveLink.hidden=!archivedCount;
    archiveLink.textContent=`恢复旧个体（${archivedCount}）`;
    const start=(page-1)*pageSize;
    grid.innerHTML=visible.length ? visible.slice(start,start+pageSize).map(renderCard).join('') : renderEmpty(rows.length>0);
    renderPages();
    if (picker.open && archiveMode) renderArchive();
  }
  function clearFilter() {search.value='';page=1;}
  function palette(query) {
    const q=normalize(query);
    if (!q) return speciesIds;
    const scores=speciesIds.map(i=>matchScore(i,q));
    return speciesIds.filter(i=>scores[i]>0).sort((a,b)=>scores[b]-scores[a]||a-b);
  }
  const splitTerms = raw => raw.split(/[、，,；;\r\n]+/).map(s=>s.trim()).filter(Boolean);
  function exact(term) {
    const q=normalize(term), candidates=speciesIds.filter(i=>matchScore(i,q)===4);
    return candidates.length===1 ? candidates[0] : null;
  }
  function batchMode(raw) { return /[、，,；;\r\n]/.test(raw); }
  function feedback(text,error=false) {
    const target=$('cl-add-feedback');
    target.textContent=text;
    target.dataset.error=String(error);
  }
  function renderChoices() {
    const raw=chooser.value.trim(), terms=splitTerms(raw);
    const confirm=$('cl-confirm');
    if (batchMode(chooser.value) && terms.length) {
      shownSpecies=[];
      const unresolved=terms.filter(term=>exact(term)===null);
      const recognized=terms.length-unresolved.length;
      choices.innerHTML=`<p class="cl-choice-hint">${unresolved.length?'待核对：'+escape(unresolved.join('、')):'每个名称会建成一张独立卡片，入库后可补录性别和词条。'}</p>`;
      confirm.textContent=`加入 ${recognized} 只`;
      confirm.disabled=!recognized;
      feedback(`已识别 ${recognized} / ${terms.length} 项`,Boolean(unresolved.length));
      return;
    }
    shownSpecies=palette(raw);
    choices.innerHTML=shownSpecies.length ? shownSpecies.map((i,n)=>`<button type="button" data-cl-species="${i}" tabindex="${n===0?0:-1}">${avatar(species[i])}<strong>${escape(species[i].zh)}</strong><small>${escape(dex(species[i]))}</small>${icon('right')}</button>`).join('') : '<p class="cl-choice-hint">没有匹配的物种，试试其他名称或编号。</p>';
    confirm.textContent='登记个体';
    confirm.disabled=!raw || (!shownSpecies.length || (exact(raw)===null && shownSpecies.length!==1));
    feedback(raw && shownSpecies.length>1 && exact(raw)===null?'请选择一个匹配的物种。':'');
  }
  function renderArchive() {
    const rows=inventory.archived?.() || [];
    $('cl-archives').innerHTML=rows.length ? rows.map(r=>{
      const pal=species[r.species];
      const passives=traitNames(r).map(t=>passiveRankIcon(t.rank)+escape(t.zh)).join('、');
      return `<button type="button" class="cl-archive-entry" data-cl-restore="${escape(r.id)}" ${r.terminalArchived?'disabled':''}>${pal?avatar(pal):''}<span><b>${escape(gameSpeciesName(r))} · ${escape(r.name)} · ${sexName(r.sex)}</b><small>${passives || '尚未登记词条'}</small></span><span>${r.terminalArchived?'切回该角色时恢复':'恢复'}</span></button>`;
    }).join('') : '<p class="cl-choice-hint">旧个体均已恢复。</p>';
  }
  function showPicker(archived=false) {
    archiveMode=archived;
    $('cl-pick-main').hidden=archived;
    $('cl-archive-panel').hidden=!archived;
    $('cl-picker-title').textContent=archived?'恢复旧个体':'添加帕鲁';
    if (archived) renderArchive();
    else { chooser.value='';renderChoices();choices.scrollTop=0; }
    if (!picker.open) picker.showModal();
    if (!archived) chooser.focus();
  }
  function closeDialog() {if (picker.open) picker.close();}
  function reveal(id, clearIfHidden) {
    const rows=inventory.list();
    const record=rows.find(row=>row.id===id);
    if (!record) return;
    let q=normalize(search.value);
    if (!matchRecord(record,q)) {
      if (!clearIfHidden) {search.focus({preventScroll:true});return;}
      clearFilter();q='';
    }
    const ordered=rows.filter(row=>matchRecord(row,q)).sort((a,b)=>a.species-b.species);
    page=1+Math.floor(ordered.findIndex(row=>row.id===id)/pageSize);
    refresh();
    const focus=[...grid.querySelectorAll('[data-cl-edit]')].find(el=>el.dataset.clEdit===id);
    focus?.focus({preventScroll:true});
    if (focus) {
      const card=focus.closest('[data-cl-id]');
      const bounds=card.getBoundingClientRect();
      if (bounds.top>=innerHeight || bounds.bottom<=0) card.scrollIntoView({block:'nearest'});
    }
  }
  function addOne(index) {
    closeDialog();
    editing={species:index,previous:new Set(inventory.list().filter(row=>row.species===index).map(row=>row.id))};
    addIndividual(index);
  }
  const editor=$('tp-profile-dialog');
  editor?.addEventListener('close',()=>{
    const state=editing;
    editing=null;
    if (!state || $('warehouse-view').hidden) return;
    if (state.id) {reveal(state.id,false);return;}
    const created=inventory.list().find(row=>row.species===state.species && !state.previous.has(row.id));
    if (created) reveal(created.id,true);
    else if (document.activeElement===document.body) $('cl-add').focus({preventScroll:true});
  });
  function accept() {
    const raw=chooser.value, terms=splitTerms(raw);
    if (!terms.length) return;
    if (!batchMode(raw)) {
      const id=exact(terms[0]) ?? (shownSpecies.length===1 ? shownSpecies[0] : null);
      if (id!==null) addOne(id);
      else feedback('请从匹配结果中选择一个物种。',true);
      return;
    }
    const additions=[], unknown=[];
    for (const term of terms) {
      const index=exact(term);
      if (index===null) unknown.push(term);
      else additions.push(index);
    }
    if (!additions.length) {feedback('请核对未识别的名称或编号。',true);return;}
    clearFilter();
    const inserted=inventory.addMany(additions);
    const count=inserted.length;
    if (unknown.length) {
      chooser.value=unknown.join('、');
      renderChoices();
      feedback(`已加入 ${count} 只，仍需核对 ${unknown.join('、')}。`,true);
      chooser.focus();
    } else {
      closeDialog();
      if (inserted.length) reveal(inserted[0].id,true);
    }
    $('cl-status').textContent=`已加入 ${count} 只帕鲁。`;
  }
  $('cl-add').addEventListener('click',()=>showPicker());
  $('cl-picker-close').addEventListener('click',closeDialog);
  $('cl-archive-open').addEventListener('click',()=>showPicker(true));
  $('cl-archives').addEventListener('click',event=>{
    const button=event.target.closest('[data-cl-restore]');
    if (!button) return;
    clearFilter();
    inventory.restore(button.dataset.clRestore);
    if (!inventory.archived().length) closeDialog();
  });
  picker.addEventListener('click',e=>{
    if (e.target!==picker) return;
    const box=picker.getBoundingClientRect();
    if (e.clientX<box.left || e.clientX>box.right || e.clientY<box.top || e.clientY>box.bottom) closeDialog();
  });
  search.addEventListener('input',()=>{page=1;refresh();});
  chooser.addEventListener('input',e=>{if (!e.isComposing) {renderChoices();choices.scrollTop=0;}});
  chooser.addEventListener('compositionend',renderChoices);
  chooser.addEventListener('keydown',e=>{
    if (e.isComposing || e.keyCode===229) return;
    if (e.key==='Enter' && !e.shiftKey) {e.preventDefault();accept();}
    else if (e.key==='ArrowDown' && shownSpecies.length) {e.preventDefault();choices.querySelector('button')?.focus();}
  });
  choices.addEventListener('click',e=>{
    const button=e.target.closest('[data-cl-species]');
    if (button) addOne(Number(button.dataset.clSpecies));
  });
  choices.addEventListener('keydown',e=>{
    const button=e.target.closest('[data-cl-species]');
    if (!button || !['ArrowDown','ArrowUp','Home','End'].includes(e.key)) return;
    e.preventDefault();
    const current=shownSpecies.indexOf(Number(button.dataset.clSpecies)), max=shownSpecies.length-1;
    const next={ArrowDown:Math.min(max,current+1),ArrowUp:Math.max(0,current-1),Home:0,End:max}[e.key];
    choices.querySelector(`[data-cl-species="${shownSpecies[next]}"]`)?.focus();
  });
  $('cl-confirm').addEventListener('click',accept);
  grid.addEventListener('click',e=>{
    const button=e.target.closest('button');
    if (!button) return;
    if (button.hasAttribute('data-cl-edit')) {
      editing={id:button.dataset.clEdit};
      editIndividual(editing.id);
    } else if (button.hasAttribute('data-cl-more')) addOne(Number(button.dataset.clMore));
    else if (button.hasAttribute('data-cl-remove')) {
      const card=button.closest('[data-cl-id]');
      const sibling=card.nextElementSibling || card.previousElementSibling;
      const nextId=sibling?.dataset.clId;
      const description=card.getAttribute('aria-label');
      inventory.remove(button.dataset.clRemove);
      $('cl-status').textContent='已移除'+description+'。';
      const nextButton=[...grid.querySelectorAll('[data-cl-edit]')].find(el=>el.dataset.clEdit===nextId);
      (nextButton || $('cl-add')).focus({preventScroll:true});
    } else if (button.dataset.clEmpty==='add') showPicker();
    else if (button.dataset.clEmpty==='clear') {clearFilter();refresh();search.focus();}
  });
  $('cl-pages').addEventListener('click',e=>{
    const button=e.target.closest('[data-cl-page]');
    if (!button || button.disabled) return;
    const max=Math.ceil(visible.length/pageSize);
    page={first:1,prev:Math.max(1,page-1),next:Math.min(max,page+1),last:max}[button.dataset.clPage];
    refresh();
    const first=grid.querySelector('[data-cl-edit]');
    first?.focus({preventScroll:true});
    grid.scrollIntoView({block:'start'});
  });
  refresh();
  function resetWorld() {
    closeDialog();editing=null;clearFilter();page=1;refresh();
  }
  return {refresh,closeDialog,resetWorld};
}

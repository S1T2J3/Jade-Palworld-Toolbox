function initTraitPlanner({species,pairs,readPairs=()=>pairs,traitData,inventory,avatar,dex,icon,escape,normalize,matchScore,solveTarget,spawnLink,spawnHref,surgeryBook,worlds,readDetails,readSpawnData}) {
  const $ = id => document.getElementById(id);
  const catalog = traitData.traits;
  const slots = traitData.maxSlots || 4;
  const allSpecies = species.map((_,index) => index);
  const sortedCatalog = [...catalog].sort(comparePassiveTraits);
  const allTraitKeys = sortedCatalog.map(trait => trait.key);
  const traitByKey = new Map(catalog.map(trait => [trait.key,trait]));
  const dialog = $('tp-profile-dialog');
  const targetInput = $('tp-target');
  const goalInput = $('tp-trait');
  const donorInput = $('tp-profile-trait');
  const requested = new Set();
  const draft = new Set();
  let selectedTarget = null;
  let donorSpecies = null;
  let editId = null;
  let stockStamp = '';
  let epoch = 0;
  let hasOutput = false;
  const plans=worlds?createBreedingPlans({worlds,inventory,species}):null;
  const gapGuide=typeof createBreedingGapGuide==='function' && readDetails && readSpawnData
    ? createBreedingGapGuide({species,traitData,readDetails,readSpawnData,escape}):null;
  const speciesIndex=new Map(species.map((pal,i)=>[pal.key,i]));
  let activeId=null, selectedWorld=plans?.worldId(), preferred=true, latest=null, recording=null, savingHatch=false, calculateQueued=false;
  const currentPlan=()=>activeId&&plans?.read(activeId);
  const formGoal=()=>({targetKey:species[selectedTarget]?.key,traits:[...requested],random:$('tp-allow-random').checked,surgery:$('tp-use-surgery').checked});
  const traitsLabel = keys => keys.map(key=>traitByKey.get(key)?.zh || key).join('、');
  const traitCaption=key=>passiveRankIcon(traitByKey.get(key)?.rank)+escape(traitByKey.get(key)?.zh||key);
  const passiveLink=key=>traitByKey.has(key)?`<a href="#details/passive/trait/${encodeURIComponent(key)}">${traitCaption(key)}</a>`:escape(key);
  const sexLabel = sex => ({M:'雄',F:'雌'})[sex] || '性别未记';
  function message(id,text,invalid=false) {
    $(id).textContent = text;
    $(id).dataset.error = String(invalid);
  }
  function initialResult() {
    $('tp-result').innerHTML = `<div class="tp-empty">${icon('branch')}<h3>等待设定目标</h3><p>先选目标帕鲁。若需继承指定词条，请在仓库个体记录中标出携带者。</p></div>`;
  }
  function invalidate(note='') {
    gapGuide?.cancel();
    syncSurgeryHint();
    latest=null;
    epoch++;
    const previous = hasOutput;
    hasOutput = false;
    $('tp-calculate').disabled = false;
    $('tp-result').setAttribute('aria-busy','false');
    if (previous) initialResult();
    message('tp-notice',previous ? note : '');
  }
  function goalChanged(note) {
    if(activeId) {
      activeId=null;preferred=false;
      renderTracking();
      message('tp-plan-message','原计划已保留。可将更改后的目标另存为计划。');
    }
    invalidate(note);
  }
  function traitMatch(key,q) {
    if (!q) return 1;
    const t = traitByKey.get(key);
    const names = [t.zh,t.en].map(normalize);
    return names.includes(q) ? 4 : names.some(s=>s.startsWith(q)) ? 3 : names.some(s=>s.includes(q)) ? 2 : 0;
  }
  function attachPicker(input,list,{keys,match,caption,choose,edit,browseAll,showAll=false}) {
    let visible = [];
    let active = 0;
    input.setAttribute('role','combobox');
    input.setAttribute('aria-autocomplete','list');
    input.setAttribute('aria-controls',list.id);
    input.setAttribute('aria-expanded','false');
    list.setAttribute('role','listbox');
    list.setAttribute('aria-label','搜索结果');
    function close() {
      list.hidden = true;
      input.setAttribute('aria-expanded','false');
      input.removeAttribute('aria-activedescendant');
    }
    function highlight() {
      const buttons = list.querySelectorAll('[data-tp-option]');
      buttons.forEach((button,i)=>{
        button.dataset.active = String(i===active);
        button.setAttribute('aria-selected',String(i===active));
      });
      if (buttons[active]) {
        input.setAttribute('aria-activedescendant',buttons[active].id);
        const item = buttons[active];
        if (item.offsetTop < list.scrollTop) list.scrollTop = item.offsetTop;
        else if (item.offsetTop+item.offsetHeight > list.scrollTop+list.clientHeight) list.scrollTop=item.offsetTop+item.offsetHeight-list.clientHeight;
      } else input.removeAttribute('aria-activedescendant');
    }
    function open() {
      if (input.disabled) {close();return;}
      const q = normalize(input.value);
      if (browseAll && !q) {close();return;}
      const matches=keys().map(key=>({key,score:match(key,q)})).filter(item=>item.score);
      if(!browseAll) matches.sort((a,b)=>b.score-a.score);
      visible=(browseAll || showAll ? matches : matches.slice(0,28)).map(item=>item.key);
      active=0;
      list.innerHTML=visible.length ? visible.map((key,i)=>{
        const [name,extra,rank] = caption(key);
        return `<button type="button" role="option" tabindex="-1" id="${list.id}-${i}" data-tp-option="${i}" class="tp-option"><b>${passiveRankIcon(rank)}${escape(name)}</b><small>${escape(extra || '')}</small></button>`;
      }).join('') : '<div class="tp-option-empty">没有匹配项，请检查名称。</div>';
      list.hidden=false;
      list.scrollTop=0;
      input.setAttribute('aria-expanded','true');
      highlight();
    }
    function commit(i) {if (visible[i] != null) {choose(visible[i]);close();}}
    function changed() {edit?.();open();}
    input.addEventListener('focus',open);
    input.addEventListener('input',e=>{if (!e.isComposing) changed();});
    input.addEventListener('compositionend',changed);
    input.addEventListener('keydown',e=>{
      if (e.isComposing || e.keyCode===229) return;
      if (e.key==='Escape' && !list.hidden) {e.preventDefault();e.stopPropagation();close();return;}
      if (e.key==='ArrowDown' || e.key==='ArrowUp') {
        e.preventDefault();
        if (browseAll && !input.value.trim()) {browseAll();return;}
        if (list.hidden) {open();return;}
        active=Math.max(0,Math.min(visible.length-1,active+(e.key==='ArrowDown'?1:-1)));
        highlight();
      }
      if (e.key==='Enter' && !list.hidden) {e.preventDefault();commit(active);}
    });
    list.addEventListener('mousedown',e=>e.preventDefault());
    list.addEventListener('click',e=>{
      const hit=e.target.closest('[data-tp-option]');
      if (hit) commit(Number(hit.dataset.tpOption));
    });
    input.addEventListener('blur',close);
    return {close,open};
  }
  const exact = (keys,score,value) => {
    const q=normalize(value);
    if (!q) return null;
    const matches=keys.filter(k=>score(k,q)===4);
    return matches.length===1 ? matches[0] : null;
  };
  function syncTargetLocation() {
    const link=$('tp-target-spawns');
    link.hidden=selectedTarget==null;
    if (!link.hidden) {
      link.href=spawnHref(selectedTarget);
      link.setAttribute('aria-label','查看'+species[selectedTarget].zh+'的刷新位置');
    }
  }
  function chooseTarget(index) {
    selectedTarget=index;
    targetInput.value=species[index].zh;
    syncTargetLocation();
    goalChanged('目标已更改，请重新计算。');
  }
  const palPicker=attachPicker(targetInput,$('tp-target-options'),{
    showAll:true,
    keys:()=>allSpecies,match:matchScore,
    caption:i=>[species[i].zh,`${dex(species[i])} · ${species[i].en}`],choose:chooseTarget,
    edit:()=>{selectedTarget=exact(allSpecies,matchScore,targetInput.value);syncTargetLocation();goalChanged('目标已更改，请重新计算。');}
  });
  const library=$('tp-trait-library'), libraryGrid=$('tp-library-grid'), librarySearch=$('tp-library-search');
  const selectedBar=$('tp-library-selected');
  const ranks=[5,4,3,2,1,'negative'];
  let editingTraits=null, returnFocus=null, quickSearch=false;

  function syncLibrarySelection() {
    const selected=editingTraits.selected;
    $('tp-library-limit').textContent='已选 '+selected.size+' / '+slots;
    selectedBar.innerHTML='';
    for (const key of selected) {
      const item=traitByKey.get(key)||{zh:key};
      const button=document.createElement('button');
      button.type='button';button.dataset.libraryRemove=key;
      button.setAttribute('aria-label','移除词条'+item.zh);
      button.innerHTML=traitCaption(key)+icon('close');
      selectedBar.append(button);
    }
    for (const button of libraryGrid.querySelectorAll('[data-trait-key]')) {
      const checked=selected.has(button.dataset.traitKey);
      button.setAttribute('aria-pressed',String(checked));
      button.setAttribute('aria-disabled',String(!checked && selected.size>=slots));
    }
  }
  function renderLibrary() {
    const query=normalize(librarySearch.value);
    const matches=query ? sortedCatalog.filter(t=>traitMatch(t.key,query)) : sortedCatalog;
    $('tp-library-count').textContent=query ? matches.length+' / '+catalog.length+' 项' : catalog.length+' 项';
    $('tp-library-clear').hidden=!librarySearch.value;
    const sections=[];
    for (const rank of ranks) {
      const group=matches.filter(t=>rank==='negative'?t.rank<0:t.rank===rank);
      if (!group.length) continue;
      const label=rank==='negative'?'负面词条':'等级 '+rank;
      const choices=group.map(t=>`<button type="button" class="tp-trait-choice" data-trait-key="${escape(t.key)}" data-rank="${t.rank}" title="${escape(t.zh+' · '+t.en)}" tabindex="-1">${traitCaption(t.key)}</button>`).join('');
      sections.push(`<section class="tp-library-group${rank==='negative'?' tp-library-negative':''}" aria-label="${label}"><h3>${label}</h3><div class="tp-library-options">${choices}</div></section>`);
    }
    libraryGrid.innerHTML=sections.join('') || '<p class="tp-library-empty">没有匹配的词条。可清除搜索查看全部。</p>';
    const first=libraryGrid.querySelector('[data-trait-key]');
    if(first) first.tabIndex=0;
    syncLibrarySelection();
  }
  function openLibrary(editor,trigger,fromInput) {
    editingTraits=editor;
    returnFocus=trigger;quickSearch=fromInput;
    editor.closeQuick();
    $('tp-library-title').textContent=editor.title;
    librarySearch.value=fromInput?editor.input.value:'';
    renderLibrary();
    library.showModal();library.scrollTop=0;
    const focusSearch=fromInput || !window.matchMedia('(max-width:650px)').matches;
    (focusSearch ? librarySearch : $('tp-library-title')).focus({preventScroll:true});
  }
  function toggleLibraryTrait(key) {
    if (!editingTraits || !traitByKey.has(key)) return false;
    const selected=editingTraits.selected;
    if (selected.has(key)) selected.delete(key);
    else if (selected.size<slots) selected.add(key);
    else return false;
    editingTraits.input.value='';
    editingTraits.render();editingTraits.onEdit();
    syncLibrarySelection();return true;
  }
  function closeLibrary(restore=true) {
    if (!library.open) return;
    const trigger=returnFocus;
    returnFocus=null;editingTraits=null;
    library.close();
    if (restore && trigger && trigger.getClientRects().length) trigger.focus({preventScroll:true});
  }
  library.addEventListener('cancel',e=>{e.preventDefault();closeLibrary();});
  library.addEventListener('click',e=>{
    if (e.target!==library) return;
    const box=library.getBoundingClientRect();
    if (e.clientX<box.left || e.clientX>box.right || e.clientY<box.top || e.clientY>box.bottom) closeLibrary();
  });
  $('tp-library-done').addEventListener('click',()=>closeLibrary());
  $('tp-library-clear').addEventListener('click',()=>{librarySearch.value='';renderLibrary();librarySearch.focus();});
  librarySearch.addEventListener('input',e=>{if(!e.isComposing) renderLibrary();});
  librarySearch.addEventListener('compositionend',renderLibrary);
  librarySearch.addEventListener('keydown',e=>{
    if (e.isComposing || e.keyCode===229) return;
    if (e.key==='ArrowDown') {e.preventDefault();libraryGrid.querySelector('[tabindex="0"]')?.focus();return;}
    if (e.key!=='Enter') return;
    e.preventDefault();
    const q=normalize(librarySearch.value);
    if (!q) {libraryGrid.querySelector('[tabindex="0"]')?.focus();return;}
    const result=exact(allTraitKeys,traitMatch,librarySearch.value) || allTraitKeys.find(key=>traitMatch(key,q));
    if (!result || !toggleLibraryTrait(result)) return;
    if (quickSearch) closeLibrary();
    else {librarySearch.value='';renderLibrary();}
  });
  selectedBar.addEventListener('click',e=>{
    const button=e.target.closest('[data-library-remove]');
    if (!button) return;
    const index=[...selectedBar.children].indexOf(button);
    toggleLibraryTrait(button.dataset.libraryRemove);
    (selectedBar.children[Math.min(index,selectedBar.children.length-1)] || librarySearch).focus({preventScroll:true});
  });
  libraryGrid.addEventListener('focusin',e=>{
    if (!e.target.matches('[data-trait-key]')) return;
    const previous=libraryGrid.querySelector('[tabindex="0"]');
    if (previous) previous.tabIndex=-1;
    e.target.tabIndex=0;
  });
  libraryGrid.addEventListener('click',e=>{
    const button=e.target.closest('[data-trait-key]');
    if (button) toggleLibraryTrait(button.dataset.traitKey);
  });
  libraryGrid.addEventListener('keydown',e=>{
    if (!e.target.matches('[data-trait-key]') || !['ArrowLeft','ArrowRight','ArrowUp','ArrowDown','Home','End'].includes(e.key)) return;
    e.preventDefault();
    const buttons=[...libraryGrid.querySelectorAll('[data-trait-key]')], index=buttons.indexOf(e.target);
    let next;
    if (e.key==='Home') next=buttons[0];
    else if (e.key==='End') next=buttons.at(-1);
    else if (e.key==='ArrowLeft'||e.key==='ArrowRight') next=buttons[index+(e.key==='ArrowRight'?1:-1)];
    else {
      const pos=e.target.getBoundingClientRect(),direction=e.key==='ArrowDown'?1:-1;
      const nearby=buttons.map(button=>({button,box:button.getBoundingClientRect()}))
        .filter(item=>(item.box.top-pos.top)*direction>2)
        .sort((a,b)=>Math.abs(a.box.top-pos.top)-Math.abs(b.box.top-pos.top) || Math.abs(a.box.left-pos.left)-Math.abs(b.box.left-pos.left));
      next=nearby[0]?.button;
    }
    next?.focus();
  });
  function wireTraits(input,listId,tagsId,slotsId,selected,onEdit,browseId,title) {
    const box=$(tagsId);
    function render() {
      $(slotsId).textContent = selected.size+' / '+slots;
      input.disabled=selected.size>=slots;
      box.innerHTML=[...selected].map(key=>
        `<button type="button" class="tp-tag" data-tp-key="${escape(key)}" aria-label="移除词条${escape(traitByKey.get(key)?.zh||key)}">${traitCaption(key)} ${icon('close')}</button>`
      ).join('');
    }
    function add(key) {
      if (!selected.has(key) && selected.size<slots) selected.add(key);
      input.value='';
      render();
      onEdit();
    }
    const picker=attachPicker(input,$(listId),{
      keys:()=>allTraitKeys.filter(k=>!selected.has(k)),match:traitMatch,
      caption:key=>[traitByKey.get(key).zh,traitByKey.get(key).en,traitByKey.get(key).rank],choose:add,
      edit:onEdit,browseAll:()=>openLibrary(editor,input,true)
    });
    const editor={input,selected,onEdit,render,title,closeQuick:picker.close};
    const browse=$(browseId);
    browse.addEventListener('click',()=>openLibrary(editor,browse,false));
    input.addEventListener('click',()=>{if (!input.value.trim()) openLibrary(editor,input,true);});
    box.addEventListener('click',e=>{
      const button=e.target.closest('[data-tp-key]');
      if (!button) return;
      const position=[...box.children].indexOf(button);
      selected.delete(button.dataset.tpKey);
      render();onEdit();
      const remaining=box.children[Math.min(position,box.children.length-1)];
      (remaining || input).focus({preventScroll:true});
      picker.close();
    });
    function commitInput() {
      if (!input.value.trim()) return true;
      const incoming=input.value.split(/[、，,；;\n]+/).map(s=>s.trim()).filter(Boolean);
      const ids=incoming.map(s=>exact(allTraitKeys,traitMatch,s));
      if (ids.some(k=>k==null) || new Set([...selected,...ids]).size>slots) return false;
      ids.forEach(k=>selected.add(k));
      input.value='';picker.close();render();onEdit();return true;
    }
    render();
    return {render,commitInput,close:()=>{picker.close();if (editingTraits===editor) closeLibrary(false);}};
  }
  const goalTags=wireTraits(goalInput,'tp-trait-options','tp-goal-tags','tp-goal-slots',requested,
    ()=>goalChanged('目标词条已更改，请重新计算。'),'tp-goal-browse','选择目标词条');
  const donorTags=wireTraits(donorInput,'tp-profile-options','tp-profile-tags','tp-profile-slots',draft,
    ()=>message('tp-profile-message',''),'tp-profile-browse','选择个体词条');
  const getStock=()=>inventory.list().filter(p=>p.species>=0);
  function syncSurgeryHint() {
    const field=$('tp-surgery-hint');
    field.hidden=!$('tp-use-surgery').checked;
    if (field.hidden) return;
    const available=surgeryBook?.options([...requested])||[];
    field.textContent=requested.size
      ? (available.length?'本次可手术补齐：'+traitsLabel(available.map(op=>op.key))+'。':'所选目标没有已登记可用的手术。')
      : '可使用手术台自带项目、已解锁项目和已标记持有的耗材。';
  }
  function refresh(suppressCalculation=false) {
    if(plans&&selectedWorld!==plans.worldId()) resetWorld();
    const stock=getStock();
    const stamp=JSON.stringify(stock.map(p=>[p.id,p.species,p.sex,p.name,p.traits]));
    if(plans && !$('planner-view').hidden && preferred) {
      preferred=false;
      const recent=plans.recent() || plans.list()[0];
      if(recent) loadPlan(recent);
      else renderTracking();
    }
    if (stamp===stockStamp) return;
    stockStamp=stamp;
    invalidate('仓库个体已更新，请重新计算。');
    $('tp-stock-count').textContent=stock.length+' 只';
    const unknown=stock.filter(row=>row.sex==='unknown').length;
    $('tp-inventory-info').textContent=!stock.length ? '先在仓库添加帕鲁并登记词条。'
      : unknown ? '有 '+unknown+' 只尚未登记性别。使用它们的路线会标出需要确认的性别。'
        : '按每只仓库帕鲁的实际性别和已登记词条计算。';
    if (dialog.open && editId && !stock.some(row=>row.id===editId)) dialog.close();
    if(activeId&&!savingHatch&&!suppressCalculation&&!$('planner-view').hidden) queueCalculate();
  }
  function openEditor(index,item=null) {
    if (!species[index]) return;
    donorSpecies=index;
    editId=item?.id||null;
    recording=null;
    $('tp-profile-expected').hidden=true;
    const pal=species[index];
    $('tp-profile-species').innerHTML=avatar(pal)+'<div><h3>'+escape(pal.zh)+'</h3><p>'+escape(dex(pal))+' · '+escape(pal.en)+'</p></div>';
    $('tp-name').maxLength=item?.id.startsWith('terminal-')?256:48;
    $('tp-name').value=item?.name||'';
    $('tp-sex').value=item?.sex||'unknown';
    draft.clear();
    for (const key of item?.traits||[]) draft.add(key);
    donorInput.value='';
    donorTags.close();
    donorTags.render();
    $('tp-dialog-title').textContent=item?'编辑仓库帕鲁':'添加仓库帕鲁';
    $('tp-save-profile').textContent=item?'保存修改':'加入仓库';
    message('tp-profile-message','');
    if (!dialog.open) dialog.showModal();
    $('tp-name').focus({preventScroll:true});
  }
  const addIndividual=index=>openEditor(index);
  function editIndividual(id) {
    const item=getStock().find(row=>row.id===id);
    if (item) openEditor(item.species,item);
  }
  async function saveProfile() {
    if(savingHatch || !dialog.open) return;
    if (!species[donorSpecies]) return;
    if (!donorTags.commitInput()) {
      message('tp-profile-message','请选择匹配的词条，每只帕鲁最多登记 4 个。',true);
      donorInput.focus();
      return;
    }
    const attributes={name:$('tp-name').value.trim(),sex:$('tp-sex').value,traits:[...draft]};
    if(recording) {
      const capture=recording;
      savingHatch=true;$('tp-save-profile').disabled=true;
      try {
        await plans.record(capture.planId,capture.world,{...capture.entry,...attributes});
        if(plans.worldId()!==capture.world || activeId!==capture.planId) return;
        dialog.close();renderTracking();refresh();await calculate();
      } catch(error) {message('tp-profile-message',error.message||'记录未写入，请重试。',true);}
      finally {savingHatch=false;$('tp-save-profile').disabled=false;}
      return;
    }
    const saved=editId ? inventory.update(editId,attributes) : inventory.add(donorSpecies,attributes);
    if (!saved) {dialog.close();return;}
    refresh();
    dialog.close();
  }
  // Keep the in-flow matching list from moving the save button between pointer down and up.
  $('tp-save-profile').addEventListener('mousedown',event=>{if(document.activeElement===donorInput) event.preventDefault();});
  $('tp-save-profile').addEventListener('click',saveProfile);
  $('tp-dialog-close').addEventListener('click',()=>dialog.close());
  $('tp-cancel-profile').addEventListener('click',()=>dialog.close());
  dialog.addEventListener('click',e=>{
    if (e.target!==dialog) return;
    const r=dialog.getBoundingClientRect();
    if (e.clientX<r.left || e.clientX>r.right || e.clientY<r.top || e.clientY>r.bottom) dialog.close();
  });
  dialog.addEventListener('close',()=>{if (!dialog.open) {donorSpecies=null;editId=null;recording=null;donorTags.close();}});
  $('tp-allow-random').addEventListener('change',()=>goalChanged('孵化条件已更改，请重新计算。'));
  $('tp-use-surgery').addEventListener('change',()=>goalChanged('手术条件已更改，请重新计算。'));
  surgeryBook?.subscribe(()=>{if ($('tp-use-surgery').checked) {invalidate('手术台记录已更改，请重新计算。');if(activeId) queueCalculate();}});
  function queueCalculate() {
    if(calculateQueued) return;
    calculateQueued=true;
    queueMicrotask(()=>{
      calculateQueued=false;
      if(activeId && !$('planner-view').hidden && !savingHatch) calculate();
    });
  }
  function planName(plan) {
    return species[speciesIndex.get(plan.targetKey)].zh+(plan.traits.length?' · '+traitsLabel(plan.traits):' · 仅物种');
  }
  function resetForm() {
    activeId=null;latest=null;selectedTarget=null;requested.clear();
    targetInput.value='';goalInput.value='';
    $('tp-allow-random').checked=false;$('tp-use-surgery').checked=false;
    goalTags.close();palPicker.close();goalTags.render();syncTargetLocation();invalidate();
    initialResult();
    if(plans) {renderTracking();message('tp-plan-message','');}
  }
  function resetWorld() {
    selectedWorld=plans.worldId();preferred=true;stockStamp='';
    if(dialog.open) dialog.close();
    recording=null;resetForm();
  }
  function loadPlan(plan) {
    preferred=false;
    if(!plan) {resetForm();return;}
    if(dialog.open) dialog.close();
    activeId=plan.id;selectedTarget=speciesIndex.get(plan.targetKey);
    plans.remember(plan.id);
    requested.clear();plan.traits.forEach(k=>requested.add(k));
    targetInput.value=species[selectedTarget].zh;goalInput.value='';
    $('tp-allow-random').checked=plan.random;$('tp-use-surgery').checked=plan.surgery;
    goalTags.close();palPicker.close();goalTags.render();syncTargetLocation();invalidate();
    message('tp-plan-message','');renderTracking();
    if(plan.pending) {
      plans.settle(plan.id,selectedWorld).then(()=>{
        if(activeId===plan.id) {renderTracking();queueCalculate();}
      }).catch(e=>message('tp-plan-message',e.message,true));
    } else queueCalculate();
  }
  function progressLabel(entry) {
    const absent=entry.expectedTraits.filter(k=>!entry.traits.includes(k));
    const notes=[];
    if(absent.length) notes.push('缺 '+traitsLabel(absent));
    if(entry.expectedSexes.length&&!entry.expectedSexes.includes(entry.sex)) notes.push(entry.sex==='unknown'?'性别待确认':'性别与本步要求不同');
    return notes.join(' · ')||'符合本步要求';
  }
  function renderTracking() {
    if(!plans) return;
    const deletion=$('tp-delete-panel');
    if(deletion.dataset.id!==activeId || deletion.dataset.world!==selectedWorld) deletion.hidden=true;
    $('tp-plans-panel').hidden=false;
    const select=$('tp-plan-select'), saved=plans.list();
    select.replaceChildren(new Option('未保存的目标',''));
    for(const p of saved) select.add(new Option(planName(p)+' · '+new Date(p.created).toLocaleDateString('zh-CN'),p.id));
    select.value=activeId||'';
    const plan=saved.find(p=>p.id===activeId);
    $('tp-plan-delete').hidden=!plan;
    $('tp-plan-save').disabled=Boolean(plan);
    $('tp-plan-save').textContent=plan?'已保存':'保存为计划';
    const box=$('tp-plan-progress');
    if(!plan) {box.replaceChildren();return;}
    const result=latest?.result;
    const status=result?.status==='owned'?(result.surgery?.length?'待完成手术':'目标已达成'):
      result?.status==='ok'?'剩余 '+result.generations+' 代':result?'待补齐条件':'待计算';
    const count=plan.log.filter(e=>e.kind==='hatch').length;
    const logs=[...plan.log].reverse().map(e=>{
      const pal=species[speciesIndex.get(e.speciesKey)];
      const line=e.kind==='hatch'?'第 '+e.generation+' 代 · 孵出':'手术后';
      return '<li><b>'+line+' '+escape(pal.zh)+'</b> · '+escape(e.name||'未命名个体')+' · '+sexLabel(e.sex)+
        '<small>'+escape(traitsLabel(e.traits)||'未登记词条')+' · '+escape(progressLabel(e))+'</small>'+
        (e.parents.length?'<small>亲代 '+e.parents.map(p=>escape(p.name)+'（'+escape(species[speciesIndex.get(p.speciesKey)].zh)+'）').join(' × ')+'</small>':'')+'</li>';
    }).join('');
    const route=plan.initialRoute;
    function originalStatus(step) {
      const attempts=plan.log.filter(e=>e.kind==='hatch' && e.speciesKey===step.speciesKey);
      const ready=attempts.filter(e=>step.traits.every(k=>e.traits.includes(k)));
      const missing=step.sexes.filter(s=>!ready.some(e=>e.sex===s));
      if(ready.length&&!missing.length) return '已孵得符合要求的子代';
      if(!attempts.length) return '尚未录入';
      return '已录 '+attempts.length+' 只 · '+(ready.length?'还需'+missing.map(sexLabel).join('、')+'性':'仍缺指定词条');
    }
    const original=route?'<details><summary>最初保存的路线 · '+route.generations+' 代</summary><ol class="tp-plan-log">'+
      route.steps.map(s=>'<li>第 '+s.generation+' 代 · '+s.parents.map(p=>escape(species[speciesIndex.get(p.speciesKey)].zh)+' '+sexLabel(p.sex)).join(' × ')+
        ' → '+escape(species[speciesIndex.get(s.speciesKey)].zh)+'<small>'+escape(traitsLabel(s.traits)||'词条不限')+
        (s.sexes.length?' · '+s.sexes.map(sexLabel).join('／'):'')+' · '+originalStatus(s)+'</small></li>').join('')+'</ol>'+
      (!route.steps.length?'<p>使用仓库已有个体。</p>':'')+(route.surgery.length?'<p>手术添加 '+escape(traitsLabel(route.surgery))+'</p>':'')+'</details>':'';
    const oldLog=box.querySelector('[data-plan-log]')?.open;
    box.innerHTML='<p><b>'+status+'</b> · 已记录 '+count+' 只子代</p>'+original+
      (logs?'<details data-plan-log'+(oldLog?' open':'')+'><summary>执行记录 · '+plan.log.length+' 条</summary><ol class="tp-plan-log">'+logs+'</ol></details>':'');
  }
  function routeSnapshot(result,stock) {
    const source=new Map(stock.map(r=>[r.id,r])),founders=new Map(result.founders.map(f=>[f.id,f.stockId]));
    const steps=new Map(result.steps.map((s,i)=>[s.id,i+1]));
    return {generations:result.generations,surgery:(result.surgery||[]).map(op=>op.key),
      steps:result.steps.map(step=>({speciesKey:species[step.species].key,generation:step.depth,traits:[...step.traits],sexes:[...step.sexes],
        parents:step.parents.map(p=>{
          const stockId=founders.get(p.id)||null;
          return {stockId,speciesKey:species[p.species].key,sex:p.sex,
            name:stockId?(source.get(stockId)?.name||'未命名个体'):('配对 '+steps.get(p.id)+' 的子代')};
        })}))};
  }
  function beginRecord(trigger) {
    const plan=currentPlan();
    if(!plan||!latest?.route) return;
    const result=latest.result, isSurgery=trigger.hasAttribute('data-plan-surgery');
    const step=latest.route.steps[Number(trigger.dataset.planHatch)];
    const actual=isSurgery?getStock().find(p=>p.id===result.founders[0]?.stockId):null;
    if(isSurgery && !actual || !isSurgery && (!step||step.parents.some(p=>!p.stockId))) return;
    const id=plans.uuid(), index=isSurgery?selectedTarget:speciesIndex.get(step.speciesKey);
    const parentIds=new Set(step?.parents.map(p=>p.stockId)||[]);
    const parentGeneration=plan.log.reduce((max,e)=>parentIds.has(e.stockId)?Math.max(max,e.generation):max,0);
    const entry={id,stockId:isSurgery?actual.id:'hatch-'+id,kind:isSurgery?'surgery':'hatch',speciesKey:species[index].key,
      at:Date.now(),generation:isSurgery?0:parentGeneration+1,expectedTraits:isSurgery?[...requested]:step.traits,
      expectedSexes:isSurgery?[]:step.sexes,parents:isSurgery?[]:step.parents};
    openEditor(index,actual);
    recording={planId:plan.id,world:plans.worldId(),entry};
    $('tp-dialog-title').textContent=isSurgery?'登记实际手术结果':'录入实际孵出的子代';
    $('tp-save-profile').textContent=isSurgery?'更新个体并重算':'子代入仓并重算';
    $('tp-profile-expected').hidden=false;
    $('tp-profile-expected').textContent='本步要求：'+(traitsLabel(entry.expectedTraits)||'词条不限')+
      (entry.expectedSexes.length?' · '+entry.expectedSexes.map(sexLabel).join('／')+'性':'')+
      '。请按游戏中这只帕鲁的实际情况填写。';
  }
  if(plans) {
    $('tp-plan-select').addEventListener('change',e=>loadPlan(e.target.value?plans.read(e.target.value):null));
    $('tp-plan-new').addEventListener('click',()=>{preferred=false;resetForm();targetInput.focus();});
    $('tp-plan-save').addEventListener('click',()=>{
      if(activeId) return;
      if(selectedTarget===null || !goalTags.commitInput()) {message('tp-plan-message','请先选定目标帕鲁，并确认目标词条。',true);return;}
      const plan=plans.create(formGoal(),latest?.route||null);
      loadPlan(plan);
      message('tp-plan-message','计划已保存。');
    });
    $('tp-plan-delete').addEventListener('click',()=>{
      const plan=currentPlan();
      if(!plan) return;
      const panel=$('tp-delete-panel');
      panel.dataset.id=plan.id;panel.dataset.world=selectedWorld;
      $('tp-delete-note').textContent='删除「'+planName(plan)+'」及其执行记录？已入仓的个体可继续使用。';
      panel.hidden=false;$('tp-delete-cancel').focus();
    });
    $('tp-delete-cancel').addEventListener('click',()=>{$('tp-delete-panel').hidden=true;$('tp-plan-delete').focus();});
    $('tp-delete-confirm').addEventListener('click',async()=>{
      const panel=$('tp-delete-panel'),id=panel.dataset.id,world=panel.dataset.world;
      if(panel.hidden || activeId!==id || selectedWorld!==world) return;
      panel.hidden=true;
      try {await plans.remove(id,world);if(activeId===id && selectedWorld===world) resetForm();}
      catch(e) {message('tp-plan-message',e.message,true);}
    });
    $('tp-result').addEventListener('click',e=>{
      const trigger=e.target.closest('[data-plan-hatch],[data-plan-surgery]');
      if(trigger) beginRecord(trigger);
    });
    worlds.events.addEventListener('storage',event=>{
      if(selectedWorld!==plans.worldId()) {resetWorld();refresh();return;}
      if(event.key!==null && !event.key.startsWith('pal-atlas.plan.v1.')) return;
      if(activeId && !plans.read(activeId)) resetForm();
      else renderTracking();
    });
  }
  function renderFailure(result,stock,randomEnabled) {
    let title, details=[];
    if (!stock.length) {
      title='仓库里还没有帕鲁';
      details=['请先到配种仓库加入已有帕鲁，再为个体登记词条。'];
    } else if (result.status==='missing-traits') {
      title='仓库缺少已知的词条来源';
      const missing=result.missing || [];
      details.push('请补录携带「'+traitsLabel(missing)+'」的个体，或调整目标词条。');
      const random=missing.filter(k=>traitByKey.get(k)?.random);
      const fixed=missing.filter(k=>!traitByKey.get(k)?.random);
      if (random.length && !randomEnabled) details.push('「'+traitsLabel(random)+'」可以随机出现，勾选“允许靠随机孵化补词条”可尝试计算。');
      if (fixed.length) details.push('常规随机词条池无法补出「'+traitsLabel(fixed)+'」，请核对已有携带者。');
      if (missing.some(key=>traitByKey.get(key)?.surgery)) details.push('有些缺少的词条可通过手术添加。可在帕鲁手术台登记持有的植入体，并勾选使用手术台词条。');
      if (result.speciesAlsoUnreachable) details.push('当前仓库的物种和性别也无法组成通往目标物种的路线。');
    } else if (result.status==='unreachable-species') {
      title='当前仓库配不出目标物种';
      details.push(species[selectedTarget]?.reason || '检查登记的雌雄性别，或加入能配出目标帕鲁的亲代。');
    } else if (result.status==='unconfirmed-sex') {
      title='请先确认部分亲代的性别';
      details.push('仓库中有多只性别未记的帕鲁。请为参与配对的个体补录性别后重新计算。');
    } else if (result.status==='unreachable-traits') {
      title='当前仓库无法把这些词条集中到目标上';
      details.push('目标物种可以获得，但现有亲代、性别及词条组合无法完成传递。可补入异性伙伴、其他携带者，或调整目标。');
      if (!randomEnabled && [...requested].some(k=>traitByKey.get(k)?.random)) details.push('也可以勾选“允许靠随机孵化补词条”后重算。');
    } else {
      title='无法计算这组目标';details=['请检查目标物种和词条后重试。'];
    }
    $('tp-result').innerHTML=`<div class="tp-empty" data-tp-status="${escape(result.status)}">${icon('search')}<h3>${title}</h3>${details.map(s=>'<p>'+escape(s)+'</p>').join('')}<p><a class="tp-text-link" href="#warehouse">管理配种仓库 ${icon('right')}</a></p></div>`;
  }
  function displayPlan(result,stock) {
    const pal=species[selectedTarget];
    const stockById=new Map(stock.map(p=>[p.id,p]));
    const foundersById=new Map(result.founders.map(f=>[f.id,stockById.get(f.stockId)]));
    const stepNumbers=new Map(result.steps.map((s,i)=>[s.id,i+1]));
    const names=[...requested].map(k=>`<span class="tp-pill">${passiveLink(k)}</span>`).join('');
    const surgery=result.surgery||[];
    const history=currentPlan()?.log||[];
    const previouslyHatched=new Map(history.filter(e=>e.kind==='hatch').map(e=>[e.stockId,e.generation]));
    const generationById=new Map(result.founders.map(f=>[f.id,previouslyHatched.get(f.stockId)||0]));
    for(const step of result.steps) generationById.set(step.id,1+Math.max(...step.parents.map(p=>generationById.get(p.id))));
    const summary=result.status==='owned' ? (surgery.length?'使用仓库现有个体 · 0 代':'仓库已有符合目标的帕鲁 · 0 代')
      : `${history.length?'从当前仓库再配':result.conditional || result.minimumUnconfirmed || result.conditionalSex ? '此路线' : '最少'} ${result.generations} 代 · ${result.steps.length} 组配对`;
    function founderLabel(source) {
      if (!source) return '仓库个体';
      return source.name || '未命名个体';
    }
    const founders=result.founders.map(f=>{
      const source=stockById.get(f.stockId);
      const gender=f.assumedSex ? '须确认是'+sexLabel(f.assumedSex)+'性' : sexLabel(f.sex);
      return `<span class="tp-founder" data-tp-founder="${escape(f.stockId)}"><b>${spawnLink(f.species)}</b> · ${escape(founderLabel(source))} · ${gender}${f.requiredTraits?.length ? ' · '+escape(traitsLabel(f.requiredTraits)) : ''}</span>`;
    }).join('');
    function parentMarkup(parent) {
      const index=stepNumbers.get(parent.id);
      const desc=index ? '配对 '+index+' 的子代' : '仓库 · '+founderLabel(foundersById.get(parent.id));
      return `<span class="tp-parent"><b>${spawnLink(parent.species)} ${sexLabel(parent.sex)}</b> <small>(${escape(desc)})</small>${parent.traits.length ? `<span class="tp-within">〔${escape(traitsLabel(parent.traits))}〕</span>` : ''}</span>`;
    }
    const rows=result.steps.map((step,i)=>{
      const keep=step.traits.length?'保留 '+traitsLabel(step.traits):'词条不限';
      const nextSex=step.sexes?.length ? ' · 留'+(step.sexes.length===2?'雌雄各一只':sexLabel(step.sexes[0])+'性') : '';
      const roll=step.randomTraits?.length?`<span class="tp-roll">需随机孵出 ${escape(traitsLabel(step.randomTraits))}</span>`:'';
      const actionable=step.parents.every(p=>foundersById.get(p.id));
      const action=activeId ? '<div class="tp-plan-step-actions">'+(actionable?
        '<button type="button" data-plan-hatch="'+i+'">录入这一步的子代</button>':'等待前序子代入仓')+'</div>':'';
      return `<li class="tp-pair" data-tp-step="${i+1}" data-tp-child="${step.species}" data-tp-depth="${step.depth}"><span class="tp-pair-index"><b>第 ${generationById.get(step.id)} 代</b>配对 ${i+1}</span><div><div class="tp-parents">${parentMarkup(step.parents[0])}<span>×</span>${parentMarkup(step.parents[1])}</div><div class="tp-child">${icon('arrow')}<strong>${spawnLink(step.species)}</strong><span>${escape(keep+nextSex)}</span>${roll}</div>${action}</div></li>`;
    }).join('');
    const sexNote=result.conditionalSex ? '部分亲代尚未登记性别，请先核对下列个体的性别要求。' : '';
    const note=sexNote+(result.status==='owned' ? (surgery.length?'选用下列个体，在手术台添加剩余目标词条。':'此个体已符合所选条件。') : (result.conditional ?
      '标记“需随机孵出”的配对要反复孵蛋，直到随机获得指定词条。每一步只采用一个随机新增的目标词条，成功所需蛋数不定。' :
      '按代执行配对。孵到同时具有所列词条、且性别符合下一代要求的子代后，再继续繁育。'));
    const fees=surgery.reduce((sum,op)=>sum+op.cost,0);
    const operations=surgery.map(op=>{
      const item=op.item?`<a href="#items/${encodeURIComponent(op.item)}">${escape(op.name)}</a>`:'';
      const payment=op.mode==='consume'?'消耗 '+item+' ×1':op.cost.toLocaleString('zh-CN')+' 金币'+(item?' · 已解锁 '+item:'');
      return `<li><b><a href="#details/surgery/trait/${encodeURIComponent(op.key)}">${traitCaption(op.key)}</a></b><span>${payment}</span></li>`;
    }).join('');
    const after=operations?`<section data-tp-surgery><h4>对目标帕鲁进行手术 · ${surgery.length} 项</h4><ul class="tp-operations">${operations}</ul><p class="tp-note">共需 ${fees.toLocaleString('zh-CN')} 金币。优先填入空槽。词条已满时，可替换一个非目标词条。</p></section>`:'';
    const trackSurgery=activeId&&surgery.length&&result.status==='owned'?'<div class="tp-plan-step-actions"><button type="button" data-plan-surgery>登记此个体的实际手术结果</button></div>':'';
    $('tp-result').innerHTML=`<div class="tp-result-main" data-tp-status="${result.status}"><div class="tp-summary">${avatar(pal)}<div><h3>${spawnLink(selectedTarget)}</h3><p>${summary}${surgery.length?' · '+surgery.length+' 项手术':''}</p></div></div>${names?'<div class="tp-tags">'+names+'</div>':''}<div class="tp-note">${note}</div><h4>用到的仓库个体</h4><div class="tp-founders">${founders}</div>${rows?'<h4>逐代配对</h4><ol class="tp-generations">'+rows+'</ol>':''}${after}${trackSurgery}</div>`;
  }
  async function calculate() {
    palPicker.close();goalTags.close();
    if (selectedTarget==null) {
      const candidate=exact(allSpecies,matchScore,targetInput.value);
      if (candidate==null) {message('tp-notice','请从匹配结果中选择目标帕鲁。',true);targetInput.focus();return;}
      chooseTarget(candidate);
    }
    if (!goalTags.commitInput()) {
      message('tp-notice','请从匹配结果中选择词条，最多可选 4 个。',true);goalInput.focus();return;
    }
    refresh(true);
    const stock=getStock();
    const run=++epoch;
    gapGuide?.cancel();
    hasOutput=true;
    const randomEnabled=$('tp-allow-random').checked;
    const request={pairs:readPairs(),speciesCount:species.length,stock,target:selectedTarget,traits:[...requested],
      surgeryOptions:$('tp-use-surgery').checked?(surgeryBook?.options([...requested])||[]):[],
      rules:traitData,allowRandom:randomEnabled,randomEligible:catalog.filter(t=>t.random).map(t=>t.key)};
    message('tp-notice','正在检查仓库中的亲代、性别和词条组合…');
    $('tp-result').setAttribute('aria-busy','true');
    $('tp-calculate').disabled=true;
    try {
      const result=stock.length ? await solveTarget(request)
        : {status:'unreachable-species',missing:[]};
      if (run!==epoch) return;
      if (result.status==='missing-traits') {
        const speciesOnly=await solveTarget({...request,traits:[],allowRandom:false});
        if (run!==epoch) return;
        result.speciesAlsoUnreachable=speciesOnly.status==='unreachable-species';
      }
      if (result.status==='ok' || result.status==='owned') {
        latest={result,route:routeSnapshot(result,stock)};
        displayPlan(result,stock);
        const plan=currentPlan();
        if(plan&&!plan.initialRoute) {
          await plans.attachRoute(plan.id,selectedWorld,latest.route);
          if(run!==epoch) return;
        }
      } else {
        latest={result,route:null};
        renderFailure(result,stock,randomEnabled);
        gapGuide?.show($('tp-result'),request,result);
      }
      renderTracking();
      const resultHeading=$('tp-result').querySelector('h3')?.textContent || '';
      const operationCount=result.surgery?.length||0;
      const report=operationCount?`${result.generations} 代，${result.steps.length} 组配对，最后进行 ${operationCount} 项手术。`
        : result.status==='ok'?`${result.generations} 代，${result.steps.length} 组配对。`
        : result.status==='owned'?'仓库已有符合条件的个体。':resultHeading+'。';
      message('tp-notice',report);
    } catch {
      if (run!==epoch) return;
      initialResult();message('tp-notice','计算中断，请重新计算。',true);
    } finally {
      if (run===epoch) {
        $('tp-calculate').disabled=false;
        $('tp-result').setAttribute('aria-busy','false');
      }
    }
  }
  $('tp-calculate').addEventListener('click',calculate);
  initialResult();refresh();
  return {refresh,addIndividual,editIndividual,closeDialog:()=>{closeLibrary(false);if(dialog.open) dialog.close();}};
}

function initWorldManager({worlds,inventory,beforeOpen,restoreFocus,prepareImport}) {
  const el=name=>document.getElementById('world-'+name);
  const dialog=el('dialog');
  const desktop=document.querySelector('meta[name="pal-desktop"][content="standalone"]')!==null;
  el('open-window').hidden=!desktop;
  if(desktop) el('open-window').addEventListener('click',()=>document.dispatchEvent(new CustomEvent('pal-new-window',{bubbles:true})));
  el('legacy').hidden=!desktop;
  el('game-scan').parentElement.hidden=!desktop;
  if(!desktop) el('game-help').textContent='Windows 独立版可以自动识别本机游戏存档。此页可在下方恢复手册 JSON 备份。';
  let gameSaves=[], gameRequest=null, gameAction=false;
  const terminalByWorld=new Map();
  let shownOwners=null, directRefresh=null;
  function refreshStatus(text,busy=false) {
    const node=document.getElementById('tp-refresh-status');
    if(node){node.hidden=!text;node.textContent=text;}
    const button=document.querySelector('[data-world-refresh]');
    if(button){button.disabled=busy;button.setAttribute('aria-busy',String(busy));}
  }
  document.addEventListener('click',event=>{
    const button=event.target.closest('[data-world-refresh]');
    if(!button || directRefresh || gameRequest) return;
    if(!desktop){refreshStatus('自动读取需要 Windows 独立版，请在仓库手动更新。');return;}
    const world=worlds.active();
    if(!world.gameId){refreshStatus('请先关联当前世界的游戏存档，之后可一键刷新。');document.querySelector('[data-world-open]')?.click();return;}
    directRefresh={worldId:world.id,gameId:world.gameId};
    refreshStatus('正在读取最新存档中的终端及随身队伍，请先在游戏内保存。',true);
    requestGameSaves('auto');
  });
  let legacyGuideOffered=false;
  let editingId=null, deletingId=null, preview=null, opener=null, fileRevision=0;
  function message(text='',error=false) {
    const node=el('feedback');
    node.textContent=text;node.hidden=!text;node.dataset.error=String(error);
  }
  function resetForms() {
    el('edit').hidden=true;el('delete-confirm').hidden=true;
    editingId=null;deletingId=null;
  }
  function clearPreview() {
    fileRevision++;preview=null;el('file').value='';
    el('preview').hidden=true;el('preview-list').replaceChildren();
  }
  function summaryLine(counts) {
    return '仓库 '+counts.individuals+' 只 · 归档 '+counts.archived+' 只 · 手术标记 '+counts.techniques+' 项 · 配种计划 '+counts.plans+' 个'+
      (counts.pins||counts.completed?' · 自定义点位 '+(counts.pins||0)+' 处 · 地图已完成 '+(counts.completed||0)+' 处':'');
  }
  function gameDetails(roots=[],warnings=[]) {
    const lines=[...roots.map(p=>'已检查 '+p),...warnings];
    const list=el('game-details');
    list.replaceChildren(...lines.map(text=>{const li=document.createElement('li');li.textContent=text;return li;}));
    el('game-diagnostics').hidden=!lines.length;
  }
  function showGameDirectory(path='') {
    const input=el('game-directory');
    input.value=typeof path==='string'?path:'';
    el('game-directory-row').hidden=!input.value;
  }
  function namedOption(label,id,selected) {
    const option=new Option(label,id,false,selected);
    option.label=label;
    return option;
  }
  function showTerminal() {
    const world=worlds.active(),job=terminalByWorld.get(world.id);
    if(directRefresh?.requestId && job?.requestId===directRefresh.requestId) {
      const busy=['reading','saving'].includes(job.phase);
      refreshStatus(job.text+(job.notes.length?' '+job.notes.join(' '):''),busy);
      if(job.phase==='choose' && !dialog.open){dialog.showModal();refresh();}
      if(!busy) directRefresh=null;
    }
    const visible=desktop && dialog.open && job && world.gameId===job.gameId && gameSaves.some(s=>s.id===job.gameId);
    el('terminal').hidden=!visible;
    if(!visible){shownOwners=null;return;}
    el('terminal-status').textContent=job.text;
    el('terminal-owner-form').hidden=job.phase!=='choose';
    el('terminal-change').hidden=!['done','missing'].includes(job.phase);
    if(job.phase==='choose' && shownOwners!==job) {
      const select=el('terminal-owner'),counts=new Map();
      for(const o of job.owners) counts.set(o.name,(counts.get(o.name)||0)+1);
      select.replaceChildren(namedOption('请选择游戏角色','',true),...job.owners.map(o=>{
        const label=o.name && counts.get(o.name)===1?o.name:(o.name||'玩家')+' · '+o.id;
        return namedOption(label,o.id,false);
      }));
      shownOwners=job;
    }
    el('terminal-confirm').disabled=!el('terminal-owner').value || job.phase!=='choose';
    const notes=el('terminal-notes');
    notes.replaceChildren(...job.notes.map(line=>{const li=document.createElement('li');li.textContent=line;return li;}));
    notes.hidden=!job.notes.length;
  }
  const canonicalOwner=value=>typeof value==='string'?value.toLowerCase().replaceAll('-',''):'';
  function requestTerminal(world,choose=false) {
    if(!desktop || (!dialog.open && directRefresh?.worldId!==world.id) || !world.gameId || !gameSaves.some(s=>s.id===world.gameId)) return;
    const previous=terminalByWorld.get(world.id);
    if(previous?.gameId===world.gameId && ['reading','saving'].includes(previous.phase)) return;
    const requestId=globalThis.crypto?.randomUUID?.()||Date.now().toString(36)+'-'+Math.random().toString(36).slice(2);
    if(directRefresh?.worldId===world.id) directRefresh.requestId=requestId;
    terminalByWorld.set(world.id,{worldId:world.id,gameId:world.gameId,requestId,started:Date.now(),priorOwner:worlds.terminalOwner(world.id,world.gameId),choose,
      phase:'reading',text:'正在读取终端里的帕鲁…',notes:[]});
    showTerminal();
    document.dispatchEvent(new CustomEvent('pal-read-terminal',{bubbles:true,detail:JSON.stringify({requestId,gameId:world.gameId})}));
  }
  async function saveTerminal(job,owner) {
    if(!owner || !job.rows || job.phase==='saving') return;
    job.phase='saving';job.text='正在写入该世界的仓库…';showTerminal();
    try {
      const result=await worlds.importTerminal(job.worldId,job.gameId,owner.id,job.rows.filter(r=>r.ownerId===owner.id),job.started,job.priorOwner);
      job.phase='done';
      job.text=result.stale?'仓库已采用较新的读档结果。':'已从「'+(owner.name||owner.id)+'」的终端读取：新增 '+result.added+' 只，更新 '+result.updated+' 只，已有 '+result.unchanged+' 只。';
      if(result.ignored) job.notes.push('已在手册移除或归档的 '+result.ignored+' 只保留原状态。');
      if(result.archived) job.notes.push('已不在当前角色终端的 '+result.archived+' 只已移到归档，不再参与配种。');
      if(result.restored) job.notes.push('已恢复这名角色先前导入的 '+result.restored+' 只。');
      if(result.unrecognized?.length) job.notes.push('尚无图鉴的物种已按游戏编号入库：'+result.unrecognized.join('、')+'。');
      if(result.unknownTraits?.length) job.notes.push('尚无译名的词条按游戏编号显示：'+result.unknownTraits.join('、')+'。');
    } catch(error) {job.phase='error';job.text='终端导入失败。'+error.message;}
    finally {job.rows=null;showTerminal();}
  }
  el('terminal-owner').addEventListener('change',showTerminal);
  el('terminal-confirm').addEventListener('click',()=>{
    const job=terminalByWorld.get(worlds.active().id);
    if(job?.phase==='choose') saveTerminal(job,job.owners.find(o=>o.id===el('terminal-owner').value));
  });
  el('terminal-change').addEventListener('click',()=>requestTerminal(worlds.active(),true));
  if(desktop) document.addEventListener('pal-terminal-result',event=>{
    let result;
    try {result=JSON.parse(event.detail);}catch{return;}
    const job=[...terminalByWorld.values()].find(j=>j.phase==='reading' && j.requestId===result?.requestId && j.gameId===result.gameId);
    if(!job) return;
    try {
      if(result.state!=='ok') throw Error(result.message||'未能读取帕鲁终端。请在游戏保存后重新检测。');
      if(!Array.isArray(result.rows) || !Array.isArray(result.owners)) throw Error('终端数据格式无效。');
      const owners=new Map();
      for(const o of result.owners) {
        const id=canonicalOwner(o?.id);
        if(!/^[0-9a-f]{32}$/.test(id)||/^0+$/.test(id)||owners.has(id)) throw Error('无法唯一识别存档中的玩家。');
        owners.set(id,{id,name:typeof o.name==='string'?o.name:'',isLocal:o.isLocal===true});
      }
      job.rows=result.rows.map(row=>{
        const ownerId=canonicalOwner(row?.ownerId);
        if(!owners.has(ownerId)) throw Error('无法确认部分帕鲁所属的玩家，请重新读取。');
        return {...row,ownerId};
      });
      job.notes=Array.isArray(result.warnings)?result.warnings.filter(s=>typeof s==='string'):[];
      job.owners=[...owners.values()];
      const locals=job.owners.filter(o=>o.isLocal);
      const owner=job.priorOwner?owners.get(job.priorOwner):(locals.length===1?locals[0]:null);
      if(owner && !job.choose){saveTerminal(job,owner);return;}
      if(!owners.size) {
        job.phase='empty';job.text='没有读到玩家的终端资料。请检查该世界的玩家存档后重新检测。';
      } else if(!job.choose && job.priorOwner && !owner) {
        job.phase='missing';job.text='本次存档缺少上次选定的角色，仓库已保留。';
      } else {
        job.phase='choose';job.text=job.choose?'选择你的游戏角色。切换后，上一位角色导入的个体会移到归档。':'请选择你在这个世界的角色。下次读档会自动沿用。';
      }
    } catch(error){job.rows=null;job.phase='error';job.text='终端读取失败。'+error.message;}
    showTerminal();
  });
  function renderGameChoice(all=worlds.list(),active=worlds.active()) {
    const save=gameSaves.find(s=>s.id===el('game-select').value);
    el('game-choices').hidden=!save;
    if(!save) return;
    const bound=all.find(w=>w.gameId===save.id), parts=[save.platform];
    const time=Date.parse(save.modified);
    if(Number.isFinite(time)) parts.push('最近保存 '+new Date(time).toLocaleString('zh-CN',{hour12:false}));
    if(save.path) parts.push(save.path);
    el('game-location').textContent=parts.filter(Boolean).join(' · ');
    el('game-open').textContent=bound?'打开已关联的档案':'新建同名档案';
    el('game-open').disabled=gameAction;
    el('game-bind').hidden=Boolean(bound||active.gameId);
    el('game-bind').disabled=gameAction;
    const hint=bound?'已关联「'+bound.name+'」。':
      active.gameId?'新档案使用游戏内世界名。':'可关联「'+active.name+'」，沿用游戏内名称和当前手册记录。';
    el('game-bind-hint').textContent=hint;
  }
  function renderGameList(previous) {
    const select=el('game-select'),selected=[previous,worlds.active().gameId].find(id=>gameSaves.some(s=>s.id===id));
    const counts=new Map();
    for(const save of gameSaves) counts.set(save.name,(counts.get(save.name)||0)+1);
    select.replaceChildren(...gameSaves.map(save=>{
      const suffix=counts.get(save.name)>1?' · '+[save.platform,save.id.slice(-8)].filter(Boolean).join(' · '):'';
      return namedOption(save.name+suffix,save.id,save.id===selected);
    }));
    if(selected) select.value=selected;
    renderGameChoice();
  }
  function requestGameSaves(mode) {
    if(!desktop || (!dialog.open && !directRefresh) || gameRequest) return;
    gameRequest=globalThis.crypto?.randomUUID?.()||Date.now().toString(36)+'-'+Math.random().toString(36).slice(2);
    dialog.dataset.gameScanRequest=gameRequest;
    el('game-scan').disabled=true;el('game-folder').disabled=true;
    el('game-status').textContent=mode==='folder'?'请选择游戏存档目录…':'正在检测本机游戏存档…';
    document.dispatchEvent(new CustomEvent('pal-read-game-saves',{bubbles:true,detail:JSON.stringify({mode,requestId:gameRequest})}));
  }
  if(desktop) document.addEventListener('pal-game-saves-result',async event=>{
    let result;
    try {result=JSON.parse(event.detail);} catch{return;}
    if(!result || (!dialog.open && !directRefresh) || !gameRequest || result.requestId!==gameRequest) return;
    const revision=gameRequest;
    const strings=list=>Array.isArray(list)?list.filter(s=>typeof s==='string'&&s):[];
    if(result.pickingDefault===true) {
      gameSaves=[];renderGameList();showTerminal();
      showGameDirectory(result.defaultDirectory);
      gameDetails(strings(result.roots),strings(result.warnings));
      el('game-status').textContent='未检测到世界。已填入默认存档目录，请在打开的窗口中选择存档文件夹。';
      return;
    }
    try {
      if(result.state==='cancelled'){
        if(!gameSaves.length && !el('game-directory').value) showGameDirectory(result.defaultDirectory);
        el('game-status').textContent=el('game-directory').value?'已取消选择，仍可从下方目录重新查找。':'已取消选择目录。';return;
      }
      const warnings=strings(result.warnings);
      gameDetails(strings(result.roots),warnings);
      if(result.state!=='ok') {
        if(!gameSaves.length) showGameDirectory(result.selectedDirectory||result.defaultDirectory||el('game-directory').value);
        el('game-status').textContent=result.message||'未能读取本机存档。可以重新检测或选择目录。';return;
      }
      if(!Array.isArray(result.saves)) throw Error('存档识别结果无效。');
      const valid=new Map();
      for(const s of result.saves) {
        if(s && typeof s.id==='string' && /^[a-zA-Z0-9_.:+-]{1,256}$/.test(s.id) && typeof s.name==='string' && s.name.trim() && s.name.length<=256) {
          valid.set(s.id,{id:s.id,name:s.name,platform:String(s.platform||''),path:String(s.path||''),modified:s.modified||''});
        }
      }
      const records=[...valid.values()].sort((a,b)=>(Date.parse(b.modified)||0)-(Date.parse(a.modified)||0)||a.name.localeCompare(b.name,'zh-CN')||a.id.localeCompare(b.id));
      await worlds.syncGameNames(records);
      if(revision!==gameRequest || (!dialog.open && !directRefresh)) return;
      const previous=el('game-select').value;
      gameSaves=records;renderGameList(previous);
      showTerminal();
      showGameDirectory(result.selectedDirectory||(!records.length?result.defaultDirectory:''));
      let status='未找到游戏世界，可以选择其他存档目录。';
      if(records.length) status='检测到 '+records.length+' 个游戏世界。';
      else if(result.selectedDirectory) status='所选目录中没有可读取的世界。可以选择其他存档目录。';
      else if(result.defaultDirectory) status=result.defaultDirectoryExists===false
        ?'本机未找到下面的默认存档目录。可以选择游戏实际保存的位置。'
        :'已检查下面的默认存档目录，未找到可读取的世界。可以选择其他存档目录。';
      el('game-status').textContent=status+(warnings.length?' 有 '+warnings.length+' 条扫描提示。':'');
      if(directRefresh && (worlds.active().id!==directRefresh.worldId || worlds.active().gameId!==directRefresh.gameId)) {
        refreshStatus('当前世界已切换，请刷新当前世界。');directRefresh=null;return;
      }
      requestTerminal(worlds.active());
    } catch(error) {
      if(revision===gameRequest) el('game-status').textContent='读取失败。'+error.message;
    } finally {
      if(revision===gameRequest){
        if(directRefresh && terminalByWorld.get(directRefresh.worldId)?.phase!=='reading' && terminalByWorld.get(directRefresh.worldId)?.phase!=='saving') {
          refreshStatus(el('game-status').textContent || '未找到当前世界，请检查存档路径后重试。');directRefresh=null;
        }
        gameRequest=null;delete dialog.dataset.gameScanRequest;
        el('game-scan').disabled=false;el('game-folder').disabled=false;
      }
    }
  });
  for(const [id,mode] of [['game-scan','auto'],['game-folder','folder']]) el(id).addEventListener('click',()=>requestGameSaves(mode));
  el('game-select').addEventListener('change',()=>renderGameChoice());
  async function openGame(linkCurrent) {
    const save=gameSaves.find(s=>s.id===el('game-select').value);
    if(!save || gameAction) return;
    gameAction=true;renderGameChoice();
    try {
      const world=await worlds.openGameSave(save,linkCurrent?worlds.active().id:null);
      refresh();el('game-status').textContent='已打开「'+world.name+'」。';
      requestTerminal(world);
    }catch(error){el('game-status').textContent=error.message;}
    finally{gameAction=false;renderGameChoice();}
  }
  el('game-open').addEventListener('click',()=>openGame(false));
  el('game-bind').addEventListener('click',()=>openGame(true));
  el('game-unlink').addEventListener('click',async()=>{
    try{await worlds.unlinkGame(worlds.active().id);refresh();el('game-status').textContent='已解除关联，手册资料保存在原档案中。';}
    catch(error){message(error.message,true);}
  });
  function refresh() {
    const active=worlds.active();
    for(const node of document.querySelectorAll('[data-world-name]')) node.textContent=active.name;
    for(const button of document.querySelectorAll('[data-world-open]')) {
      button.title='世界档案 · '+active.name;
      button.setAttribute('aria-label','打开世界档案，当前为'+active.name);
    }
    if(!dialog.open) return;
    showTerminal();
    const all=worlds.list();
    const names=new Map();
    for(const w of all) names.set(w.name,1+(names.get(w.name)||0));
    el('select').replaceChildren(...all.map(w=>{
      const marker=names.get(w.name)>1?' · '+(w.gameId?[w.gamePlatform,w.gameId.slice(-8)].filter(Boolean).join(' · '):'手册档案'):'';
      return namedOption(w.name+marker,w.id,w.id===active.id);
    }));
    el('rename').hidden=!!active.gameId;
    el('game-bound').hidden=!active.gameId;
    el('game-bound-name').textContent=active.gameId?'已关联游戏存档 · '+(active.gamePlatform||active.gameId.slice(-8)):'';
    renderGameChoice(all,active);
    el('remove').disabled=all.length<2;
    el('remove').title=all.length<2?'至少保留一个世界':'删除当前世界';
    const counts=worlds.summary(active.id);
    el('counts').textContent=summaryLine(counts);
    if(desktop && !legacyGuideOffered) {
      el('legacy').open=all.length===1 && [counts.individuals,counts.archived,counts.techniques,counts.plans].every(n=>n===0);
      legacyGuideOffered=true;
    }
  }
  document.addEventListener('click',event=>{
    const button=event.target.closest('[data-world-open]');
    if(!button || dialog.open) return;
    opener=button;beforeOpen?.();
    resetForms();clearPreview();
    dialog.showModal();
    if(desktop) requestGameSaves('auto');
    refresh();
    message(!worlds.isPersistent()?'当前浏览器无法持久保存。请导出备份保留本次数据。':button.hasAttribute('data-world-refresh')?(desktop?'正在重新检测存档并同步当前世界的终端；请先在游戏中保存。未关联世界时，请在下方选择存档。':'自动读取游戏终端需要 Windows 独立版；也可在仓库手动更新个体。'):'',!worlds.isPersistent());
  });
  el('close').addEventListener('click',()=>dialog.close());
  dialog.addEventListener('close',()=>{
    resetForms();clearPreview();
    gameRequest=null;gameSaves=[];el('game-select').replaceChildren();el('game-choices').hidden=true;gameDetails();
    delete dialog.dataset.gameScanRequest;
    showGameDirectory();
    el('game-scan').disabled=false;el('game-folder').disabled=false;el('game-status').textContent='';
    if(opener) restoreFocus?.(opener);
    opener=null;
  });
  dialog.addEventListener('click',e=>{
    if(e.target!==dialog) return;
    const r=dialog.getBoundingClientRect();
    if(e.clientX<r.left||e.clientX>r.right||e.clientY<r.top||e.clientY>r.bottom) dialog.close();
  });
  el('select').addEventListener('change',()=>{
    try{worlds.select(el('select').value);message('已切换到「'+worlds.active().name+'」。');}
    catch(error){refresh();message(error.message,true);}
  });
  function edit(rename) {
    resetForms();message();
    const active=worlds.active();
    editingId=rename?active.id:null;
    el('name-label').textContent=rename?'为当前世界重命名':'新世界名称';
    el('save').textContent=rename?'保存':'创建并切换';
    el('name').value=rename?active.name:'';
    el('edit').hidden=false;el('name').focus();el('name').select();
  }
  el('new').addEventListener('click',()=>edit(false));
  el('rename').addEventListener('click',()=>edit(true));
  el('cancel').addEventListener('click',()=>{resetForms();el('select').focus();});
  el('edit').addEventListener('submit',event=>{
    event.preventDefault();
    const name=el('name').value.trim();
    if(!name){message('请输入世界名称。',true);el('name').focus();return;}
    try{
      if(editingId) worlds.rename(editingId,name);else worlds.create(name);
      resetForms();refresh();message('当前世界为「'+worlds.active().name+'」。');el('select').focus();
    }catch(error){message(error.message,true);}
  });
  el('remove').addEventListener('click',()=>{
    resetForms();message();
    const active=worlds.active();deletingId=active.id;
    el('delete-note').textContent='删除「'+active.name+'」及其仓库、手术台记录和配种计划？';
    el('delete-confirm').hidden=false;el('delete-no').focus();
  });
  el('delete-no').addEventListener('click',()=>{resetForms();el('select').focus();});
  el('delete-yes').addEventListener('click',()=>{
    if(!deletingId) return;
    try{worlds.remove(deletingId);resetForms();refresh();message('世界档案已删除。');el('select').focus();}
    catch(error){message(error.message,true);}
  });
  el('export').addEventListener('click',()=>{
    try{
      const scope=el('export-scope').value, backup=worlds.exportBackup(scope);
      const label=scope==='all'?'全部世界':worlds.active().name.replace(/[\\/:*?"<>|\u0000-\u001f]/g,'_');
      const name='Jade 幻兽帕鲁工具箱-'+label+'-'+new Date().toISOString().slice(0,10)+'.json';
      if(desktop) {
        message('请选择 JSON 备份的保存位置。');
        const save=new CustomEvent('pal-save-backup',{bubbles:true,cancelable:true,detail:JSON.stringify({name,backup})});
        document.dispatchEvent(save);
        return;
      }
      const blob=new Blob([JSON.stringify(backup,null,2)],{type:'application/json;charset=utf-8'});
      const url=URL.createObjectURL(blob), anchor=document.createElement('a');
      anchor.download=name;
      anchor.href=url;document.body.append(anchor);
      try{anchor.click();}finally{anchor.remove();setTimeout(()=>URL.revokeObjectURL(url),15000);}
      message('已生成'+(scope==='all'?'全部世界':'当前世界')+'的 JSON 备份。');
    }catch(error){message(error.message,true);}
  });
  function chooseBackup(){el('file').value='';el('file').click();}
  if(desktop) document.addEventListener('pal-backup-result',event=>{
    const result=JSON.parse(event.detail);
    if(typeof result.text==='string') message(result.text,result.state==='error');
    if(dialog.open) el('export').focus();
  });
  el('import').addEventListener('click',chooseBackup);
  el('legacy-import').addEventListener('click',chooseBackup);
  el('legacy-open').addEventListener('click',event=>{
    if(!desktop || !event.isTrusted) return;
    document.dispatchEvent(new CustomEvent('pal-open-legacy',{bubbles:true}));
    message('导出旧资料后，请回到这里选择 JSON 备份。');
  });
  el('file').addEventListener('change',async()=>{
    const file=el('file').files[0];
    if(!file) return;
    clearPreview();const revision=fileRevision;message('正在读取备份…');
    try{
      const [raw]=await Promise.all([file.text(),prepareImport?.()]);
      if(revision!==fileRevision) return;
      const inspected=worlds.previewImport(raw);
      const rows=inspected.worlds.map(world=>{
        const li=document.createElement('li'), label=document.createElement('strong'), counts=document.createElement('span');
        label.textContent=world.name;counts.className='world-preview-detail';counts.textContent=summaryLine(world);
        li.append(label,counts);return li;
      });
      el('preview-summary').textContent=file.name+' · '+rows.length+' 个世界，将分别创建副本。';
      el('preview-list').replaceChildren(...rows);
      preview=inspected;el('preview').hidden=false;message();el('restore').focus();
    }catch(error){if(revision===fileRevision) message('导入失败。'+error.message,true);}
  });
  el('discard').addEventListener('click',()=>{clearPreview();message();el('import').focus();});
  el('restore').addEventListener('click',()=>{
    if(!preview) return;
    try{
      const added=worlds.importBackup(preview);
      clearPreview();refresh();message('已导入 '+added.length+' 个世界，当前为「'+worlds.active().name+'」。');
      el('select').focus();
    }catch(error){message('导入失败。'+error.message,true);}
  });
  worlds.subscribe(change=>{if(change.type==='switch'){resetForms();if(!gameAction) requestTerminal(change.world);}refresh();});
  inventory.subscribe(()=>{if(dialog.open)refresh();});
  worlds.events.addEventListener('storage',event=>{
    if(dialog.open && (event.key?.startsWith('pal-atlas.surgery.v1.') || event.key?.startsWith('pal-atlas.plan.v1.') || event.key?.startsWith('pal-atlas.map.v1.'))) refresh();
  });
  refresh();
}

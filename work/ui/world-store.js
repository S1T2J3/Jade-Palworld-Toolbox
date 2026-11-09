function createPalWorlds({species,traitData,readPassives,validatePlan,storage,session,events=globalThis.window}) {
  const prefix='pal-atlas.worlds.v1.', inventoryKey='pal-atlas.inventory.v2';
  const surgeryPrefix='pal-atlas.surgery.v1.', planPrefix='pal-atlas.plan.v1.';
  const mapPrefix='pal-atlas.map.v1.';
  const removedPrefix=inventoryKey+'.removed.';
  const groups=prefix+'group.', renamed=prefix+'name.', deleted=prefix+'deleted.', gameLinks=prefix+'game.';
  const speciesSet=new Set(species.map(p=>p.key)), traits=new Set(traitData.traits.map(t=>t.key));
  const maxTraits=traitData.maxSlots||4;
  const format='pal-atlas-worlds', defaultId='default';
  const callbacks=new Set(), previews=new WeakMap(), seeds=new Map();
  const scopeEvents=new EventTarget();
  const memory=new Map(), cache=new Map();
  let durable=true,writeLocked=false;
  if(storage===undefined) {try {storage=globalThis.localStorage;} catch {storage=null;}}
  if(session===undefined) {try {session=globalThis.sessionStorage;} catch {session=null;}}
  try {if(storage) void storage.length;} catch {storage=null;}
  durable=Boolean(storage);
  const uid=()=>globalThis.crypto?.randomUUID?.()||Date.now().toString(36)+'-'+Math.random().toString(36).slice(2);
  const isObject=x=>x!==null && typeof x==='object' && !Array.isArray(x);
  const dataPrefix=id=>prefix+'data.'+id+'.';
  const seedKey=id=>prefix+'seed.'+id;
  function logical(k) {
    return typeof k==='string' && (k===inventoryKey || k.startsWith(inventoryKey+'.') ||
      k==='pal-atlas.warehouse.v1' || k==='pal-atlas.trait-profiles.v1' ||
      k.startsWith(surgeryPrefix) || k.startsWith(planPrefix) || k.startsWith(mapPrefix));
  }
  function get(key) {
    if(memory.has(key)) return memory.get(key);
    try {
      const value=storage?.getItem(key)??null;
      if(value===null) cache.delete(key);else cache.set(key,value);
      return value;
    } catch {durable=false;return cache.get(key)??null;}
  }
  function put(key,value,strict=false) {
    if(storage&&!writeLocked) try {
      if(value===null) storage.removeItem(key);else storage.setItem(key,value);
      memory.delete(key);
      if(value===null) cache.delete(key);else cache.set(key,value);
      return;
    } catch(error) {
      if(strict&&error?.name!=='SecurityError') throw new Error('未能写入本机存储。请检查浏览器存储空间后再试。');
      if(error?.name==='SecurityError') writeLocked=true;
      durable=false;
    }
    memory.set(key,value);
  }
  function keys() {
    let physical=[];
    try {
      if(typeof Storage!=='undefined' && storage instanceof Storage) {
        physical=Object.getOwnPropertyNames(storage);
      } else if(storage) {
        const length=storage.length;
        for(let i=0;i<length;i++){const key=storage.key(i);if(key!==null)physical.push(key);}
      }
    } catch {durable=false;physical=[...cache.keys()];}
    if(!memory.size)return physical;
    const found=new Set(physical);
    for(const [k,v] of memory) {if(v===null) found.delete(k);else found.add(k);}
    return [...found];
  }
  function parse(value,fallback) {
    try {return JSON.parse(value)??fallback;} catch {return fallback;}
  }
  function list() {
    const all=new Map([[defaultId,{id:defaultId,name:'默认世界'}]]), links=[];
    for(const k of keys()) {
      if(k.startsWith(groups)) {
        const row=parse(get(k),[]);
        if(Array.isArray(row)) for(const w of row) if(isObject(w)&&typeof w.id==='string') all.set(w.id,{id:w.id,name:w.name});
      } else if(k.startsWith(gameLinks)) {
        const link=parse(get(k),null);
        if(link && typeof link.worldId==='string') links.push([k.slice(gameLinks.length),link]);
      }
    }
    for(const w of all.values()) w.name=get(renamed+w.id)||w.name;
    for(const [gameId,link] of links) if(link.created || all.has(link.worldId)) {
      all.set(link.worldId,{id:link.worldId,name:link.name,gameId,gamePlatform:link.platform});
    }
    return [...all.values()].filter(w=>!get(deleted+w.id));
  }
  function requireWorld(id) {
    const w=list().find(w=>w.id===id);
    if(!w) throw new Error('找不到这个世界档案。');
    return w;
  }
  function checkedName(text,max) {
    if(typeof text!=='string' || !text.trim() || text.length>max) throw new Error('世界名称须为 1～'+max+' 个字符。');
    return text;
  }
  const manualName=text=>checkedName(typeof text==='string'?text.trim():text,64);
  function uniqueName(name,taken) {
    let value=name,i=2;
    while(taken.has(value)) {
      const suffix='（'+i+++'）';
      value=name.slice(0,64-suffix.length)+suffix;
    }
    taken.add(value);
    return value;
  }
  // The old logical keys remain the physical keys of the original world.
  // Imported bundles are immutable seeds; later operations still use the
  // per-individual keys of the inventory store, preserving concurrent edits.
  function seed(id) {
    if(seeds.has(id)) return seeds.get(id);
    const result=new Map(), data=id===defaultId?null:parse(get(seedKey(id)),null);
    if(data) {
      result.set(inventoryKey,JSON.stringify({records:data.inventory.records,archived:data.inventory.archived,limit:data.inventory.limit}));
      for(const rid of data.inventory.removed||[]) result.set(removedPrefix+rid,'true');
      for(const k of data.surgery) result.set(surgeryPrefix+k,'1');
      for(const p of data.plans) result.set(planPrefix+p.id,JSON.stringify(p));
      for(const id of data.mapNotes?.completed||[]) result.set(mapPrefix+id,'1');
      for(const pin of data.mapNotes?.custom||[]) result.set(mapPrefix+pin.id,JSON.stringify(pin));
    }
    seeds.set(id,result);
    return result;
  }
  const tombstone='!removed';
  function scoped(worldId) {
    const actual=(id,k)=>id===defaultId?k:dataPrefix(id)+k;
    let lastKeys=[];
    function scopedKeys(startsWith='') {
      const id=worldId(), ns=dataPrefix(id), seeded=seed(id);
      const result=new Set([...seeded.keys()].filter(k=>k.startsWith(startsWith)));
      for(const physical of keys()) {
        const k=id===defaultId?physical:physical.startsWith(ns)?physical.slice(ns.length):null;
        if(!logical(k)||!k.startsWith(startsWith)) continue;
        if(id!==defaultId&&get(physical)===tombstone) result.delete(k);else result.add(k);
      }
      return [...result];
    }
    return {
      get worldId(){return worldId();},
      get persistent(){return durable;},
      keys:scopedKeys,
      get length(){lastKeys=scopedKeys();return lastKeys.length;},
      key(i){return lastKeys[i]??null;},
      getItem(k){
        if(!logical(k)) return null;
        const id=worldId(), v=get(actual(id,k));
        return v===tombstone?null:v??seed(id).get(k)??null;
      },
      setItem(k,v){if(!logical(k)) throw new Error('无效的世界存储键。');put(actual(worldId(),k),String(v));},
      setItemStrict(k,v){if(!logical(k)) throw new Error('无效的世界存储键。');put(actual(worldId(),k),String(v),true);},
      removeItem(k,strict=false){
        if(!logical(k)) return;
        const id=worldId();
        put(actual(id,k),seed(id).has(k)?tombstone:null,strict);
      }
    };
  }
  let preferred;
  try {preferred=session?.getItem(prefix+'active');} catch { /* A single-window session can still select worlds. */ }
  function available() {
    const all=list();
    if(all.length) return all;
    // Two windows could remove each other's last remaining world.
    const w={id:'world-'+uid(),name:'新世界'};
    put(groups+uid(),JSON.stringify([w]));return [w];
  }
  const initial=available();
  let activeId=initial.find(w=>w.id===preferred)?.id || initial.find(w=>w.id===get(prefix+'last'))?.id || initial[0].id;
  try {session?.setItem(prefix+'active',activeId);} catch { /* Other tabs keep their own current world. */ }
  const adapter=scoped(()=>activeId);
  function active(){return requireWorld(activeId);}
  function emit(type){const change={type,world:active()};for(const fn of callbacks) fn(change);}
  function notifyStorage(key,worldChange=false) {
    const ev=new Event('storage');
    Object.assign(ev,{key,worldChange});scopeEvents.dispatchEvent(ev);
  }
  function select(id) {
    const next=requireWorld(id);
    if(id===activeId) return next;
    activeId=id;
    try {session?.setItem(prefix+'active',id);} catch { /* Session storage is optional. */ }
    put(prefix+'last',id);
    notifyStorage(null,true);emit('switch');
    return next;
  }
  function create(name) {
    name=manualName(name);
    if(list().some(w=>w.name===name)) throw new Error('这个世界名称已经存在。');
    const w={id:'world-'+uid(),name};
    put(groups+uid(),JSON.stringify([w]),true);
    return select(w.id);
  }
  function rename(id,name) {
    if(requireWorld(id).gameId) throw new Error('此档案沿用游戏内世界名。请先解除关联，再手动重命名。');
    name=manualName(name);
    if(list().some(w=>w.id!==id&&w.name===name)) throw new Error('这个世界名称已经存在。');
    put(renamed+id,name,true);emit('list');return {id,name};
  }
  function gameSource(save) {
    if(!isObject(save)||typeof save.id!=='string'||!/^[a-zA-Z0-9_.:+-]{1,256}$/.test(save.id)) throw new Error('无法识别此游戏存档的编号。');
    return {id:save.id,name:checkedName(save.name,256),platform:typeof save.platform==='string'?save.platform.slice(0,80):''};
  }
  function gameLock(action) {
    const run=()=>Promise.resolve().then(action);
    return globalThis.navigator?.locks ? navigator.locks.request('pal-atlas-game-links',run) : run();
  }
  function syncGameNames(saves) {
    return gameLock(()=>{
      let changed=false;
      for(const save of saves) {
        const source=gameSource(save),key=gameLinks+source.id,link=parse(get(key),null);
        if(link && !get(deleted+link.worldId) && (link.name!==source.name || link.platform!==source.platform)) {
          put(key,JSON.stringify({...link,name:source.name,platform:source.platform}),true);changed=true;
        }
      }
      if(changed) emit('list');
    });
  }
  function openGameSave(save,targetId=null) {
    const source=gameSource(save);
    return gameLock(()=>{
      const key=gameLinks+source.id,old=parse(get(key),null);
      const existing=old && list().find(w=>w.id===old.worldId && w.gameId===source.id);
      if(existing && targetId && existing.id!==targetId) throw new Error('该存档已关联另一个手册档案，请打开已关联档案。');
      const target=targetId?requireWorld(targetId):null;
      if(target?.gameId && target.gameId!==source.id) throw new Error('当前档案已有游戏存档。请先解除原关联。');
      // One key publishes both the newly created archive and its link. A retry
      // uses the same archive. Its data namespace is independent of its name.
      const id=existing?.id||target?.id||'world-'+uid();
      const link={...(existing?old:{}),worldId:id,name:source.name,platform:source.platform,created:existing?old.created:!target};
      put(key,JSON.stringify(link),true);
      if(id!==activeId) select(id);else emit('list');
      return requireWorld(id);
    });
  }
  function importTerminal(worldId,gameId,ownerId,rows,started,priorOwner) {
    return gameLock(()=>{
      const world=requireWorld(worldId);
      if(world.gameId!==gameId) throw Error('游戏存档关联已变化，请重新读取。');
      if(!/^[0-9a-f]{32}$/.test(ownerId) || !Array.isArray(rows) || rows.some(row=>row.ownerId!==ownerId)) throw Error('终端个体所属的玩家不一致。');
      const inv=inventoryFromImage(scoped(()=>worldId));
      const result=inv.importTerminal(gameId,ownerId,rows,started,priorOwner);
      if(worldId===activeId) notifyStorage(inventoryKey+'.terminal.'+gameId);
      return result;
    });
  }
  function terminalOwner(worldId,gameId) {
    const stored=scoped(()=>worldId).getItem(inventoryKey+'.terminal.'+gameId);
    return parse(stored,null)?.ownerId||null;
  }
  const builtinPin=id=>typeof id==='string' && /^(palpagos|worldtree)-[a-z0-9_-]{1,145}$/.test(id);
  const personalPin=id=>typeof id==='string' && /^custom-[a-z0-9-]{5,90}$/.test(id);
  function checkedPin(pin) {
    if(!isObject(pin) || !personalPin(pin.id) || !['palpagos','worldtree'].includes(pin.map) ||
       ![pin.u,pin.v].every(n=>Number.isFinite(n)&&n>=0&&n<=1) ||
       typeof pin.name!=='string' || !pin.name.trim() || pin.name.length>80 ||
       typeof pin.note!=='string' || pin.note.length>600 || typeof pin.done!=='boolean')
      throw Error('自定义点位需要地图上的有效位置、1～80 字的名称和 600 字以内的备注。');
    return {id:pin.id,map:pin.map,u:pin.u,v:pin.v,name:pin.name.trim(),note:pin.note,done:pin.done};
  }
  function readMapNotes(scope) {
    const completed=[],custom=[];
    for(const key of scope.keys(mapPrefix)) {
      if(!key.startsWith(mapPrefix)) continue;
      const id=key.slice(mapPrefix.length),value=scope.getItem(key);
      if(builtinPin(id) && value==='1') completed.push(id);
      if(personalPin(id)) {
        const pin=checkedPin(parse(value,null));
        if(pin.id!==id) throw Error('自定义点位编号不一致。');
        custom.push(pin);
      }
    }
    return {completed,custom};
  }
  function updateMapNote(worldId,action) {
    return gameLock(()=>{
      if(activeId!==worldId) throw Error('当前世界已切换，请在该世界重新选择点位。');
      const key=mapPrefix+action.id, raw=adapter.getItem(key);
      if(personalPin(action.id)) {
        const old=raw===null?null:checkedPin(parse(raw,null));
        if(action.type!=='add' && !old) throw Error('该自定义点位已删除。');
        if(action.type==='delete') adapter.removeItem(key,true);
        else {
          let pin;
          if(action.type==='add' && !old) pin={...action.fields,id:action.id,done:false};
          else if(action.type==='edit' && old) pin={...old,...action.fields,id:old.id,map:old.map,done:old.done};
          else if(action.type==='done' && typeof action.done==='boolean') pin={...old,done:action.done};
          else throw Error('无法保存点位。');
          adapter.setItemStrict(key,JSON.stringify(checkedPin(pin)));
        }
      } else if(builtinPin(action.id) && action.type==='done' && typeof action.done==='boolean') {
        if(action.done) adapter.setItemStrict(key,'1');else adapter.removeItem(key,true);
      } else throw Error('无法识别这个地图点位。');
      notifyStorage(key);
      return action.id;
    });
  }
  function unlinkGame(id) {
    return gameLock(()=>{
      const w=requireWorld(id);
      if(!w.gameId) return w;
      const key=gameLinks+w.gameId,link=parse(get(key),null);
      if(!link||link.worldId!==id) throw new Error('游戏存档关联已变化，请重试。');
      // Keep the archive reachable before releasing a link that created it.
      if(link.created) put(groups+'unlinked.'+id,JSON.stringify([{id,name:w.name}]),true);
      put(renamed+id,w.name,true);
      put(key,null,true);emit('list');return requireWorld(id);
    });
  }
  function remove(id) {
    const all=list(),world=requireWorld(id);
    if(all.length<2) throw new Error('至少需要保留一个世界档案。');
    put(deleted+id,'1',true);
    if(world.gameId) put(gameLinks+world.gameId,null);
    const ns=dataPrefix(id);
    for(const k of keys()) if(id===defaultId?logical(k):k.startsWith(ns)) put(k,null);
    put(seedKey(id),null);seeds.delete(id);put(renamed+id,null);
    if(activeId===id) select(available()[0].id);else emit('list');
  }

  function snapshot(world,stable=false) {
    const scope=scoped(()=>world.id);
    const scan=()=>new Map(scope.keys().map(k=>[k,scope.getItem(k)]).filter(([,v])=>v!==null));
    let rows=scan();
    if(stable) {
      for(let tries=0;;tries++) {
        const next=scan();
        if(next.size===rows.size&&[...rows].every(([k,v])=>next.get(k)===v)) break;
        if(tries>=3) throw new Error('另一个窗口正在更新档案，请稍后重新导出。');
        rows=next;
      }
    }
    // Materialize an isolated key image so exporting a v1 archive never
    // writes or migrates any source-world record.
    const image={
      keys:()=>[...rows.keys()], getItem:k=>rows.get(k)??null, setItem:(k,v)=>rows.set(k,v),
      get length(){return rows.size;},key:i=>[...rows.keys()][i]??null
    };
    const inv=inventoryFromImage(image);
    const flat=record=>({id:record.id,speciesKey:record.speciesKey,name:record.name,sex:record.sex,traits:record.traits,legacy:!!record.legacy,
      ...(record.terminalSpeciesKey?{terminalSpeciesKey:record.terminalSpeciesKey}:{}),
      ...(record.terminalOwnerId?{terminalOwnerId:record.terminalOwnerId}:{}),...(record.terminalArchived?{terminalArchived:true}:{})});
    const records=inv.list().map(flat), archived=inv.archived().map(flat);
    const removed=[],surgery=[],plans=[];
    for(const [k,v] of rows) {
      if(k.startsWith(removedPrefix)) removed.push(k.slice(removedPrefix.length));
      if(k.startsWith(surgeryPrefix) && v==='1') surgery.push(k.slice(surgeryPrefix.length));
      if(k.startsWith(planPrefix)) {
        const plan=parse(v,null);
        if(!plan || plan.id!==k.slice(planPrefix.length)) throw new Error('计划记录损坏，无法生成完整备份。');
        plans.push(plan);
      }
    }
    const mapNotes=readMapNotes(image);
    return {id:world.id,name:world.name,inventory:{records,archived,removed,limit:inv.getLimit()},surgery,plans,
      ...(mapNotes.completed.length||mapNotes.custom.length?{mapNotes}:{})};
  }
  function inventoryFromImage(image){
    const factory=typeof createPalInventory==='function'?createPalInventory:require('./inventory-store.js').createPalInventory;
    return factory({species,traitData,storage:image,events:null});
  }
  function exportBackup(mode='current') {
    const worlds=mode==='all'?list():[active()];
    return {format,version:1,activeWorldId:activeId,worlds:worlds.map(w=>snapshot(w,true))};
  }
  function counts(w) {
    return {name:w.name,individuals:w.inventory.records.length,archived:w.inventory.archived.length,
      techniques:w.surgery.length,plans:w.plans.length,...(w.mapNotes?{pins:w.mapNotes.custom.length,
        completed:w.mapNotes.completed.length+w.mapNotes.custom.filter(p=>p.done).length}:{})};
  }
  const goodId=id=>typeof id==='string' && id.length>0 && !id.includes('\u0000');
  function insist(ok,message){if(!ok) throw new Error(message);}
  function validateWorld(w) {
    insist(isObject(w)&&goodId(w.id),'备份中的世界编号无效。');
    const name=checkedName(w.name,256), inv=w.inventory;
    insist(isObject(inv)&&Array.isArray(inv.records)&&Array.isArray(inv.archived),'仓库或归档格式不正确。');
    insist(inv.limit==='all'||(['number','string'].includes(typeof inv.limit)&&/^[1-9]\d*$/.test(inv.limit)&&Number(inv.limit)<=species.length),'仓库步数筛选无效。');
    const ids=new Set();
    const record=r=>{
      insist(isObject(r)&&goodId(r.id)&&!ids.has(r.id),'仓库个体的编号无效或重复。');
      ids.add(r.id);
      const terminal=r.id.startsWith('terminal-'),gameKey=k=>typeof k==='string'&&k.length>0&&k.length<=160&&!/[\u0000-\u001f]/.test(k);
      insist((speciesSet.has(r.speciesKey)||terminal&&gameKey(r.speciesKey))&&['M','F','unknown'].includes(r.sex),'仓库个体的物种或性别无效。');
      insist(typeof r.name==='string' && r.name.length<=(terminal?256:48),'仓库个体名称无效。');
      insist(r.legacy===undefined||typeof r.legacy==='boolean','仓库个体的迁移标记无效。');
      insist(Array.isArray(r.traits)&&r.traits.length<=maxTraits&&new Set(r.traits).size===r.traits.length&&r.traits.every(t=>traits.has(t)||terminal&&gameKey(t)),'仓库词条无效。');
      insist(r.terminalOwnerId===undefined || (terminal && /^[0-9a-f]{32}$/.test(r.terminalOwnerId)),'终端个体的玩家编号无效。');
      insist(r.terminalSpeciesKey===undefined || terminal&&gameKey(r.terminalSpeciesKey),'终端物种编号无效。');
      insist(r.terminalArchived===undefined || (r.terminalArchived===true && r.terminalOwnerId),'终端个体的归档标记无效。');
      return {id:r.id,speciesKey:r.speciesKey,name:r.name,sex:r.sex,traits:[...r.traits],legacy:r.legacy===true,
        ...(r.terminalSpeciesKey?{terminalSpeciesKey:r.terminalSpeciesKey}:{}),
        ...(r.terminalOwnerId?{terminalOwnerId:r.terminalOwnerId}:{}),...(r.terminalArchived?{terminalArchived:true}:{})};
    };
    const records=inv.records.map(record),archived=inv.archived.map(record);
    const removed=inv.removed===undefined?[]:inv.removed;
    insist(Array.isArray(removed)&&new Set(removed).size===removed.length&&removed.every(k=>goodId(k)&&!ids.has(k)),'仓库删除记录无效。');
    const passives=readPassives();
    insist(Array.isArray(w.surgery)&&new Set(w.surgery).size===w.surgery.length&&w.surgery.every(k=>typeof k==='string' && ['unlock','consume'].includes(passives[k]?.surgery?.mode)),'手术台持有记录无效。');
    const plans=w.plans;
    insist(Array.isArray(plans)&&plans.every(p=>isObject(p)&&goodId(p.id)&&validatePlan?.(p))&&new Set(plans.map(p=>p.id)).size===plans.length,'配种计划或孵化进度无效。');
    const existing=new Map([...records,...archived].map(r=>[r.id,r.speciesKey]));
    for(const p of plans) {
      const event=p.pending;
      if(event && existing.has(event.stockId)) {
        insist(existing.get(event.stockId)===event.speciesKey,'待提交的计划进度与仓库个体不匹配。');
      }
    }
    let mapNotes;
    if(w.mapNotes!==undefined) {
      insist(isObject(w.mapNotes)&&Array.isArray(w.mapNotes.custom)&&Array.isArray(w.mapNotes.completed),'地图标记格式无效。');
      const custom=w.mapNotes.custom.map(checkedPin),completed=w.mapNotes.completed;
      insist(new Set(custom.map(p=>p.id)).size===custom.length&&completed.every(builtinPin)&&new Set(completed).size===completed.length,'地图点位编号无效或重复。');
      mapNotes={custom,completed:[...completed]};
    }
    return {id:w.id,name,inventory:{records,archived,removed:[...removed],limit:String(inv.limit)},surgery:[...w.surgery],plans,...(mapNotes?{mapNotes}:{})};
  }
  function previewImport(input) {
    let b;
    try {b=JSON.parse(typeof input==='string'?input.replace(/^\uFEFF/,''):JSON.stringify(input));} catch {throw new Error('读取失败，请选择有效的 JSON 备份文件。');}
    insist(isObject(b)&&b.format===format && b.version===1,'无法识别备份格式或版本。');
    insist(Array.isArray(b.worlds)&&b.worlds.length>0,'备份没有世界档案。');
    const imported=b.worlds.map(validateWorld);
    const ids=new Set(imported.map(w=>w.id));
    insist(ids.size===imported.length&&ids.has(b.activeWorldId),'备份中的世界编号重复，或缺少原活动世界。');
    const preview={worlds:imported.map(counts)};
    previews.set(preview,{worlds:imported,active:b.activeWorldId});return preview;
  }
  function importBackup(preview) {
    const pending=previews.get(preview);
    if(!pending) throw new Error('请先重新预览备份文件。');
    const taken=new Set(list().map(w=>w.name)), written=[], incoming=[];
    let chosen;
    // Readers see staged world seeds only after the single manifest write.
    // An error rolls back all staged keys and leaves existing worlds untouched.
    try {
      for(const w of pending.worlds) {
        const id='world-'+uid(), name=uniqueName(w.name,taken);
        put(seedKey(id),JSON.stringify({inventory:w.inventory,surgery:w.surgery,plans:w.plans,mapNotes:w.mapNotes}),true);
        written.push(seedKey(id));incoming.push({id,name});
        if(w.id===pending.active) chosen=id;
      }
      put(groups+uid(),JSON.stringify(incoming),true);
    } catch(error) {for(const k of written)put(k,null);throw error;}
    previews.delete(preview);select(chosen||incoming[0].id);return incoming;
  }
  events?.addEventListener?.('storage',event=>{
    const key=event.key;
    if(key===null || key?.startsWith(groups) || key?.startsWith(renamed) || key?.startsWith(deleted) || key?.startsWith(gameLinks)) {
      if(key===null){cache.clear();seeds.clear();}
      const worlds=available();
      if(!worlds.some(w=>w.id===activeId)) select(worlds[0].id);
      else {if(key===null) notifyStorage(null);emit('list');}
      return;
    }
    const ns=dataPrefix(activeId);
    if(activeId===defaultId && logical(key)) notifyStorage(key);
    else if(key?.startsWith(ns)) notifyStorage(key.slice(ns.length));
  });
  return {storage:adapter,events:scopeEvents,active,list,create,rename,remove,select,openGameSave,syncGameNames,unlinkGame,importTerminal,terminalOwner,
    mapNotes:()=>readMapNotes(adapter),isMapPointDone:id=>builtinPin(id)&&adapter.getItem(mapPrefix+id)==='1',
    updateMapNote,newMapPinId:()=>'custom-'+uid(),
    exportBackup,previewImport,importBackup,summary:(id=activeId)=>counts(snapshot(requireWorld(id))),isPersistent:()=>durable,
    subscribe(fn){callbacks.add(fn);return ()=>callbacks.delete(fn);}};
}
if(typeof module!=='undefined'&&module.exports) module.exports={createPalWorlds};

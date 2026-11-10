function createPalInventory({species, traitData, storage, events = globalThis.window}) {
  const prefix = 'pal-atlas.inventory.v2';
  const batchPrefix = prefix + '.add.';
  const editPrefix = prefix + '.edit.';
  const removedPrefix = prefix + '.removed.';
  const terminalPrefix = prefix + '.terminal.';
  const limitKey = prefix + '.limit';
  const oldStockKey = 'pal-atlas.warehouse.v1';
  const oldProfilesKey = 'pal-atlas.trait-profiles.v1';
  const bySpecies = new Map(species.map((pal,index) => [pal.key,index]));
  const traitKeys = new Set(traitData.traits.map(trait => trait.key));
  const gameSpecies=new Map(species.map(pal=>[pal.key.toLowerCase(),pal]));
  const gameTraits=new Map(traitData.traits.map(t=>[t.key.toLowerCase(),t.key]));
  const maxTraits = traitData.maxSlots || 4;
  const listeners = new Set();
  const unsaved = new Map();
  const lastRead = new Map();
  let active = new Map();
  let countsBySpecies = new Map();
  let dormant = new Map();
  let cap = '1';
  let lastOrder = 0;
  let signature;
  let baseline;
  let durable = true;

  if (storage === undefined) {
    try { storage = globalThis.localStorage; }
    catch { storage = null; }
  }
  durable = Boolean(storage);

  function parse(value) {
    if (value == null) return null;
    try { return JSON.parse(value); }
    catch { return null; }
  }
  function read(key) {
    if (unsaved.has(key)) return unsaved.get(key);
    try {
      const value=storage?.getItem(key) ?? null;
      if (value!==null) lastRead.set(key,value);
      else lastRead.delete(key);
      return value;
    } catch { durable=false; return lastRead.get(key) ?? null; }
  }
  function write(key,value) {
    const text = JSON.stringify(value);
    try {
      if (!storage) throw new Error('Storage unavailable');
      storage.setItem(key,text);
      unsaved.delete(key);
      lastRead.set(key,text);
    } catch {
      unsaved.set(key,text);
      durable=false;
    }
  }
  function allKeys() {
    const keys = new Set(unsaved.keys());
    try {
      if (typeof storage?.keys==='function') {
        for (const key of storage.keys(prefix+'.')) if(key.startsWith(prefix+'.')) keys.add(key);
      } else if (storage) {
        const count=storage.length;
        for (let i=0;i<count;i++) {
          const key=storage.key(i);
          if (key?.startsWith(prefix+'.')) keys.add(key);
        }
      }
    } catch {
      durable=false;
      for (const key of lastRead.keys()) if (key.startsWith(prefix+'.')) keys.add(key);
    }
    return keys;
  }
  function validCap(value) {
    return value === 'all' || (/^[1-9]\d*$/.test(value) && Number(value)<=species.length);
  }
  function validRecord(record) {
    return record && typeof record.id==='string' && record.id.length>0 &&
      (bySpecies.has(record.speciesKey) || (record.id.startsWith('terminal-') && typeof record.speciesKey==='string' && record.speciesKey.length>0)) &&
      ['M','F','unknown'].includes(record.sex) && Array.isArray(record.traits);
  }
  function publicRecord(record) {
    return {...record, species:bySpecies.get(record.speciesKey)??-1, traits:[...record.traits]};
  }
  function uniqueId() {
    return globalThis.crypto?.randomUUID?.() || Date.now().toString(36)+'-'+Math.random().toString(36).slice(2);
  }

  function migrate(stock,profileStore) {
    const owned = new Set(Array.isArray(stock?.owned) ? stock.owned.filter(key=>bySpecies.has(key)) : []);
    const profiles = Array.isArray(profileStore?.profiles) ? profileStore.profiles : [];
    const reserved = new Set(profiles.map(profile=>profile?.id).filter(id=>typeof id === 'string'));
    const used = new Set();
    const groups = new Map();
    function derivedId(base) {
      let id=base, serial=1;
      while (reserved.has(id) || used.has(id)) id=base+'-'+(++serial);
      used.add(id);
      return id;
    }
    for (const profile of profiles) {
      if (!profile || typeof profile.id!=='string' || !profile.id || used.has(profile.id) ||
          !bySpecies.has(profile.speciesKey) || !['M','F','any'].includes(profile.sex) || !Array.isArray(profile.traits)) continue;
      const {speciesKey,id}=profile;
      used.add(id);
      const record={id,speciesKey,name:String(profile.name || '个体 1').slice(0,48),
        sex:profile.sex==='any' ? 'M' : profile.sex,
        traits:[...new Set(profile.traits.filter(key=>traitKeys.has(key)))].slice(0,maxTraits),legacy:false};
      if (!groups.has(speciesKey)) groups.set(speciesKey,[]);
      groups.get(speciesKey).push(record);
      if (profile.sex==='any') groups.get(speciesKey).push({...record,id:derivedId(id+'-F'),sex:'F'});
    }
    const records=[];
    for (const speciesKey of owned) {
      if (groups.has(speciesKey)) records.push(...groups.get(speciesKey));
      else records.push({id:derivedId('legacy-'+speciesKey),speciesKey,name:'个体 1',sex:'unknown',traits:[],legacy:true});
    }
    const archived=[];
    for (const [speciesKey,rows] of groups) if (!owned.has(speciesKey)) archived.push(...rows);
    return {records,archived,limit:validCap(stock?.limit)?String(stock.limit):'1'};
  }

  function loadBase(requireBase) {
    const raw=read(prefix);
    if (raw!==null) return parse(raw) || {};
    const stock=read(oldStockKey), profiles=read(oldProfilesKey);
    const imported=migrate(parse(stock),parse(profiles));
    if (requireBase || stock!==null || profiles!==null) write(prefix,imported);
    return imported;
  }
  // Import the previous two stores in a single write. Each later add (including
  // a batch), edit, or deletion has its own key, so tabs editing unrelated
  // individuals cannot overwrite each other's updates. Tombstones beat stale edits.
  function refresh(requireBase=false) {
    baseline=loadBase(requireBase);
    const members=new Map(), archived=new Map(), batches=[], terminal=[], savedArchives=[], edits=[], removed=[];
    const insert=(map,record)=>{if (validRecord(record)) map.set(record.id,record);};
    if (Array.isArray(baseline.records)) baseline.records.forEach(row=>insert(members,row));
    if (Array.isArray(baseline.archived)) baseline.archived.forEach(row=>insert(archived,row));
    for (const key of allKeys()) {
      if (key.startsWith(batchPrefix)) {
        const batch=parse(read(key));
        if (Number.isFinite(batch?.order) && Array.isArray(batch.items)) batches.push({key,...batch});
      } else if (key.startsWith(terminalPrefix)) {
        const saved=parse(read(key));
        if(Array.isArray(saved?.items)) terminal.push(...saved.items);
        if(Array.isArray(saved?.archived)) savedArchives.push(...saved.archived);
      } else if (key.startsWith(editPrefix)) {
        const row=parse(read(key));
        if (validRecord(row) && editPrefix+row.id===key) edits.push(row);
      } else if (key.startsWith(removedPrefix)) removed.push(key.slice(removedPrefix.length));
    }
    batches.sort((a,b)=>a.order-b.order || a.key.localeCompare(b.key));
    lastOrder=batches.at(-1)?.order || 0;
    for (const batch of batches) batch.items.forEach(row=>insert(members,row));
    for (const row of savedArchives) {members.delete(row.id);insert(archived,row);}
    for (const row of terminal) {
      if(archived.has(row.id) && !archived.get(row.id).terminalArchived) continue;
      archived.delete(row.id);insert(members,row);
    }
    for (const row of edits) {
      if(archived.get(row.id)?.terminalArchived) continue;
      const imported=members.get(row.id);
      if(imported?.terminalStamp && row.terminalStamp!==imported.terminalStamp) continue;
      if (members.has(row.id) || archived.has(row.id)) members.set(row.id,row);
      archived.delete(row.id);
    }
    for (const id of removed) {members.delete(id); archived.delete(id);}
    const savedCap=parse(read(limitKey)) || baseline.limit;
    cap=validCap(savedCap) ? String(savedCap) : '1';
    active=members;
    countsBySpecies=new Map();
    for(const row of members.values()) {
      const index=bySpecies.get(row.speciesKey);
      countsBySpecies.set(index,(countsBySpecies.get(index)||0)+1);
    }
    dormant=archived;
    const next=JSON.stringify([[...active.values()],[...dormant.keys()],cap,durable&&storage?.persistent!==false]);
    if (next===signature) return false;
    signature=next;
    for (const callback of listeners) callback();
    return true;
  }
  function fields(data,previous) {
    const sex=data.sex ?? previous.sex;
    if (!['M','F','unknown'].includes(sex)) throw new TypeError('请选择有效的性别。');
    const traits=data.traits ?? previous.traits;
    if (!Array.isArray(traits) || traits.some(key=>!traitKeys.has(key) && !previous.traits?.includes(key)) || new Set(traits).size>maxTraits) {
      throw new TypeError('请从词条表中选择最多 '+maxTraits+' 个词条。');
    }
    const name=String(data.name ?? previous.name).trim().slice(0,previous.id?.startsWith('terminal-')?256:48) || previous.name;
    return {name,sex,traits:[...new Set(traits)]};
  }

  function importTerminal(gameId,ownerId,rows,started,priorOwner) {
    if(typeof gameId!=='string' || !/^[a-zA-Z0-9_.:+-]{1,256}$/.test(gameId) || !Array.isArray(rows) || !Number.isFinite(started)) throw Error('终端数据格式无效。');
    refresh();
    const key=terminalPrefix+gameId, previous=parse(read(key));
    if(previous?.started>started) return {stale:true};
    if(previous?.ownerId!==ownerId && (previous?.ownerId||null)!==priorOwner) throw Error('当前档案关联的玩家已变化，请重新读取。');
    const saved=new Map((previous?.items||[]).map(row=>[row.id,row]));
    const archived=new Map((previous?.archived||[]).map(row=>[row.id,row]));
    const result={added:0,updated:0,unchanged:0,ignored:0,archived:0,restored:0,unrecognized:[],unknownTraits:[]};
    const unknownSpecies=new Set(),unknownTraits=new Set(),seen=new Map(),stamp=uniqueId();
    const recordPrefix='terminal-'+gameId+'-';
    for(const row of active.values()) {
      if(row.id.startsWith(recordPrefix) && row.terminalOwnerId && row.terminalOwnerId!==ownerId) {
        saved.delete(row.id);archived.set(row.id,{...row,terminalArchived:true,terminalStamp:stamp});result.archived++;
      }
    }
    for(const row of rows) {
      const uuid=String(row?.instanceId||'').replaceAll('-','').toLowerCase();
      if(!/^[0-9a-f]{32}$/.test(uuid) || /^0+$/.test(uuid) || typeof row.speciesKey!=='string' ||
         !row.speciesKey || row.speciesKey.length>160 || typeof row.name!=='string' || row.name.length>256 || !['M','F','unknown'].includes(row.sex) ||
         !Array.isArray(row.traits) || row.traits.length>maxTraits ||
         !row.traits.every(t=>typeof t==='string' && t.length>0 && t.length<=160 && !/[\u0000-\u001f]/.test(t))) throw Error('终端个体资料不完整。请在游戏保存后重试。');
      const source=row.speciesKey.toLowerCase();
      const base=source.replace(/^(?:(?:boss|raid|predator|gym|tower|summon|oilrig)_)+/i,'');
      const pal=gameSpecies.get(source)||gameSpecies.get(base)||gameSpecies.get(base.replace(/_(?:otomo|bossrush|quest|quest_(?:friend|enemy)|tower)$/,''));
      if(!pal) unknownSpecies.add(row.speciesKey);
      const id=recordPrefix+uuid;
      const traits=[...new Set(row.traits.filter(t=>t.toLowerCase()!=='none').map(t=>gameTraits.get(t.toLowerCase())||t))];
      const display=Object.hasOwn(traitData.terminalNames||{},source)?traitData.terminalNames[source]:pal?.zh||row.speciesKey;
      const record={id,speciesKey:pal?.key||row.speciesKey,name:row.name||display,sex:row.sex,traits,legacy:false,terminalOwnerId:ownerId,terminalSpeciesKey:row.speciesKey};
      const signature=JSON.stringify(record);
      if(seen.has(id)) {
        if(seen.get(id)!==signature) throw Error('同一终端个体出现冲突记录，请等待游戏保存完成后重试。');
        continue;
      }
      seen.set(id,signature);
      if((dormant.has(id)&&!dormant.get(id).terminalArchived)||read(removedPrefix+id)!==null){result.ignored++;continue;}
      if(dormant.get(id)?.terminalArchived) result.restored++;
      // A traded individual keeps its game UUID across player terminals.
      if(archived.delete(id) && active.get(id)?.terminalOwnerId!==ownerId && active.has(id)) result.archived--;
      for(const t of traits) if(!traitKeys.has(t)) unknownTraits.add(t);
      const old=active.get(id)||saved.get(id);
      if(old && old.terminalOwnerId===ownerId && old.terminalSpeciesKey===record.terminalSpeciesKey && old.speciesKey===record.speciesKey && old.name===record.name && old.sex===record.sex &&
        old.traits.length===traits.length && old.traits.every((t,i)=>t===traits[i])) {
        result.unchanged++;
        continue;
      }
      if(old) result.updated++;else result.added++;
      saved.set(id,{...record,terminalStamp:stamp});
    }
    // This is a complete snapshot of this player's terminal, not an additive import.
    // Keep missing individuals archived for history, but never usable as parents.
    for(const row of saved.values()) {
      if(read(removedPrefix+row.id)!==null) {saved.delete(row.id);continue;}
      if(row.terminalOwnerId===ownerId && !seen.has(row.id)) {
        saved.delete(row.id);
        archived.set(row.id,{...row,terminalArchived:true,terminalStamp:stamp});
        result.archived++;
      }
    }
    result.unrecognized=[...unknownSpecies];result.unknownTraits=[...unknownTraits];
    // One write publishes the complete merge. An older scan cannot overwrite a
    // newer read, and manual inventory operations remain on independent keys.
    const value=JSON.stringify({started,ownerId,items:[...saved.values()],archived:[...archived.values()]});
    if(storage?.setItemStrict) storage.setItemStrict(key,value);
    else if(storage) storage.setItem(key,value);
    else throw Error('仓库存储不可用，请重试。');
    refresh();
    return result;
  }
  function createBatch(indices,data={},stableId) {
    refresh(true);
    if (!Array.isArray(indices) || indices.some(index=>!Number.isInteger(index) || !species[index])) throw new TypeError('请选择有效的帕鲁。');
    if (!indices.length) return [];
    if (stableId!==undefined) {
      if (typeof stableId!=='string' || !stableId.startsWith('hatch-') || stableId.length<=6) throw new TypeError('孵化记录编号无效。');
      if (read(removedPrefix+stableId)!==null || dormant.has(stableId)) return [];
      const previous=active.get(stableId);
      if (previous) {
        if (previous.speciesKey!==species[indices[0]].key) throw new Error('这个孵化编号已用于另一只帕鲁。');
        return [publicRecord(previous)];
      }
    }
    const names=new Map(), nextNumber=new Map();
    for (const row of active.values()) {
      if (!names.has(row.speciesKey)) names.set(row.speciesKey,new Set());
      names.get(row.speciesKey).add(row.name);
    }
    const records=indices.map(index=>{
      const speciesKey=species[index].key;
      const usedNames=names.get(speciesKey) || new Set();
      let number=nextNumber.get(speciesKey) || 1;
      while (usedNames.has('个体 '+number)) number++;
      const details=fields(data,{name:'个体 '+number,sex:'unknown',traits:[]});
      usedNames.add(details.name); names.set(speciesKey,usedNames);
      nextNumber.set(speciesKey,number);
      return {id:stableId||'pal-'+uniqueId(),speciesKey,...details,legacy:false};
    });
    write(batchPrefix+records[0].id,{order:Math.max(Date.now(),lastOrder+1),items:records});
    refresh();
    if(stableId) return active.has(stableId)?[publicRecord(active.get(stableId))]:[];
    return records.map(publicRecord);
  }

  refresh();
  events?.addEventListener?.('storage',event=>{
    if(event.worldChange) {unsaved.clear();lastRead.clear();signature=undefined;durable=Boolean(storage);}
    if (event.key===null || event.key===prefix || event.key?.startsWith(prefix+'.') ||
        event.key===oldStockKey || event.key===oldProfilesKey) refresh();
  });
  return {
    list:()=>[...active.values()].map(publicRecord),
    archived:index=>[...dormant.values()].filter(row=>index===undefined || bySpecies.get(row.speciesKey)===index).map(publicRecord),
    has:index=>countsBySpecies.has(index),
    count:index=>countsBySpecies.get(index)||0,
    get persistent() {return durable && storage?.persistent!==false;},
    add:(index,data={})=>createBatch([index],data)[0],
    addOnce:(index,data,stableId)=>{
      if(!stableId) throw new TypeError('需要孵化记录编号。');
      return createBatch([index],data,stableId)[0]??null;
    },
    addMany:indices=>createBatch(indices),
    importTerminal,
    update(id,patch={}) {
      refresh(true);
      const previous=active.get(id);
      if (!previous || previous.terminalArchived) return null;
      const updated={...previous,...fields(patch,previous),legacy:false};
      write(editPrefix+id,updated);
      refresh();
      return active.has(id) ? publicRecord(active.get(id)) : null;
    },
    remove(id) {
      refresh(true);
      if (!active.has(id)) return false;
      write(removedPrefix+id,true);
      refresh();
      return true;
    },
    restore(id) {
      refresh(true);
      const previous=dormant.get(id);
      if (!previous || previous.terminalArchived) return null;
      write(editPrefix+id,previous);
      refresh();
      return active.has(id) ? publicRecord(active.get(id)) : null;
    },
    getLimit:()=>cap,
    setLimit(value) {
      if (!validCap(value)) return false;
      refresh(true);
      if (cap!==String(value)) {write(limitKey,String(value));refresh();}
      return true;
    },
    refresh:()=>refresh(),
    subscribe(callback) {listeners.add(callback); return ()=>listeners.delete(callback);}
  };
}
if (typeof module!=='undefined' && module.exports) module.exports={createPalInventory};

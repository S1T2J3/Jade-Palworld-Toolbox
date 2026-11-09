function createSurgeryCollection(readPassives, storage, events=globalThis.window) {
  const prefix='pal-atlas.surgery.v1.';
  const localChanges=new Map();
  const listeners=new Set();
  if (storage===undefined) {
    try {storage=globalThis.localStorage;} catch {storage=null;}
  }
  let persistent=Boolean(storage);
  function has(key) {
    const operation=readPassives()[key]?.surgery;
    if (!operation) return false;
    if (operation.mode==='coins') return true;
    if (localChanges.has(key)) return localChanges.get(key);
    try {return storage?.getItem(prefix+key)==='1';}
    catch {persistent=false;return false;}
  }
  function notify() {for (const fn of listeners) fn();}
  function set(key,owned) {
    const operation=readPassives()[key]?.surgery;
    if (!operation || operation.mode==='coins' || has(key)===owned) return;
    try {
      if (!storage) throw new Error('Storage unavailable');
      if (owned) storage.setItem(prefix+key,'1');
      else storage.removeItem(prefix+key);
      localChanges.delete(key);
    } catch {persistent=false;localChanges.set(key,owned);}
    notify();
  }
  events?.addEventListener('storage',event=>{
    if(event.worldChange) localChanges.clear();
    if (event.key===null || event.key.startsWith(prefix)) notify();
  });
  function options(keys) {
    const catalog=readPassives();
    return keys.filter(has).map(key=>({key,...catalog[key].surgery}));
  }
  return {has,set,options,subscribe:fn=>listeners.add(fn),isPersistent:()=>persistent&&storage?.persistent!==false};
}

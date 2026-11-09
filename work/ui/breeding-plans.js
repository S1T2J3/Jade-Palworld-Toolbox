// The persisted route uses game IDs so a backup is independent of the current dex order.
function validateBreedingPlan(plan,species,traitData) {
  const pals=new Set(species.map(p=>p.key)), allTraits=new Set(traitData.traits.map(t=>t.key));
  const object=v=>v!==null && typeof v==='object' && !Array.isArray(v);
  const text=(v,n)=>typeof v==='string' && v.length<=n;
  const id=v=>text(v,180)&&v.length>0;
  const integer=(n,min,max=Number.MAX_SAFE_INTEGER)=>Number.isSafeInteger(n)&&n>=min&&n<=max;
  const keys=(v,terminal=false)=>Array.isArray(v)&&v.length<=4&&new Set(v).size===v.length&&v.every(k=>allTraits.has(k)||(terminal&&text(k,160)&&k.length>0));
  const sexes=v=>Array.isArray(v)&&v.length<=2&&new Set(v).size===v.length&&v.every(s=>s==='M'||s==='F');
  const parent=p=>object(p)&&pals.has(p.speciesKey)&&['M','F'].includes(p.sex)&&(p.stockId===null||id(p.stockId))&&text(p.name,p.stockId?.startsWith('terminal-')?256:100);
  const pair=v=>Array.isArray(v)&&v.length===2&&v.every(parent);
  const step=s=>object(s)&&pals.has(s.speciesKey)&&integer(s.generation,1)&&keys(s.traits)&&sexes(s.sexes)&&pair(s.parents);
  const route=r=>r===null || (object(r)&&integer(r.generations,0)&&keys(r.surgery)&&Array.isArray(r.steps)&&r.steps.every(step));
  const entry=e=>object(e)&&id(e.id)&&id(e.stockId)&&pals.has(e.speciesKey)&&integer(e.at,0)&&integer(e.generation,0)&&
    text(e.name,e.stockId.startsWith('terminal-')?256:48)&&['M','F','unknown'].includes(e.sex)&&keys(e.traits,e.stockId.startsWith('terminal-'))&&keys(e.expectedTraits)&&sexes(e.expectedSexes)&&
    ((e.kind==='hatch'&&e.stockId==='hatch-'+e.id&&e.generation>=1&&pair(e.parents)) ||
    (e.kind==='surgery'&&Array.isArray(e.parents)&&!e.parents.length));
  return object(plan)&&id(plan.id)&&pals.has(plan.targetKey)&&keys(plan.traits)&&typeof plan.random==='boolean'&&typeof plan.surgery==='boolean'&&
    integer(plan.created,0)&&route(plan.initialRoute)&&Array.isArray(plan.log)&&plan.log.every(entry)&&new Set(plan.log.map(e=>e.id)).size===plan.log.length&&
    (plan.pending===null || (entry(plan.pending)&&!plan.log.some(e=>e.id===plan.pending.id)));
}

function createBreedingPlans({worlds,inventory,species}) {
  const storage=worlds.storage, prefix='pal-atlas.plan.v1.';
  const indexByKey=new Map(species.map((p,i)=>[p.key,i]));
  const uuid=()=>globalThis.crypto?.randomUUID?.()||Date.now().toString(36)+'-'+Math.random().toString(36).slice(2);
  const worldId=()=>worlds.active().id;
  function read(id) {
    const raw=storage.getItem(prefix+id);
    return raw?JSON.parse(raw):null;
  }
  function put(plan) {storage.setItem(prefix+plan.id,JSON.stringify(plan));return plan;}
  function list() {
    const plans=[];
    const keys=storage.keys?storage.keys(prefix):Array.from({length:storage.length},(_,i)=>storage.key(i));
    for(const key of keys) {
      if(key?.startsWith(prefix)) {
        const plan=read(key.slice(prefix.length));
        if(plan) plans.push(plan);
      }
    }
    return plans.sort((a,b)=>b.created-a.created || a.id.localeCompare(b.id));
  }
  async function locked(id,world,fn) {
    const run=async()=>{
      if(worldId()!==world) throw new Error('世界已切换，请在当前世界重新打开计划。');
      const plan=read(id);
      if(!plan) throw new Error('这个计划已删除。');
      return fn(plan);
    };
    return globalThis.navigator?.locks
      ? navigator.locks.request('pal-atlas-plan/'+world+'/'+id,run) : run();
  }
  function applyPending(plan) {
    const entry=plan.pending;
    if(!entry) return plan;
    const attrs={name:entry.name,sex:entry.sex,traits:entry.traits};
    if(entry.kind==='surgery') {
      const individual=inventory.list().find(p=>p.id===entry.stockId);
      if(individual && individual.speciesKey!==entry.speciesKey) throw new Error('待登记的手术与仓库个体的物种不匹配。');
    }
    const record=entry.kind==='surgery'?inventory.update(entry.stockId,attrs):
      inventory.addOnce(indexByKey.get(entry.speciesKey),attrs,entry.stockId);
    // A deleted individual must not reappear when replaying an interrupted save.
    if(record) entry.name=record.name;
    plan.log.push(entry);
    plan.pending=null;
    return put(plan);
  }
  const recentKey=()=> 'pal-atlas.ui.plan-last.v1.'+worldId();
  return {
    list,read,uuid,worldId,
    recent() {
      try {return read(globalThis.localStorage?.getItem(recentKey())||'');}
      catch {return null;}
    },
    remember(id) {
      try {globalThis.localStorage?.setItem(recentKey(),id);} catch { /* Selection can be recovered from the plan list. */ }
    },
    create(goal,initialRoute) {
      return put({id:'plan-'+uuid(),created:Date.now(),...goal,initialRoute,log:[],pending:null});
    },
    settle(id,world) {return locked(id,world,applyPending);},
    attachRoute(id,world,route) {
      return locked(id,world,plan=>plan.initialRoute?plan:put({...plan,initialRoute:route}));
    },
    record(id,world,entry) {
      return locked(id,world,plan=>{
        applyPending(plan);
        if(plan.log.some(e=>e.id===entry.id)) return plan;
        plan.pending=entry;
        put(plan);
        return applyPending(plan);
      });
    },
    remove(id,world) {return locked(id,world,plan=>{applyPending(plan);storage.removeItem(prefix+id);});}
  };
}
if(typeof module!=='undefined'&&module.exports) module.exports={validateBreedingPlan,createBreedingPlans};

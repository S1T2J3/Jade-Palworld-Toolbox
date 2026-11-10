const assert=require('node:assert/strict');
const {createPalWorlds}=require('../ui/world-store');
const {createPalInventory}=require('../ui/inventory-store');
const {species}=require('../data/breeding.json');
const traitData={...require('../data/traits.json'),terminalNames:require('../ui/terminal-names.json')};
const {validateBreedingPlan}=require('../ui/breeding-plans');
class Local {
  constructor(){this.values=new Map()}
  getItem(k){return this.values.get(k)??null}
  setItem(k,v){if(this.reject?.(k))throw Error('quota');this.values.set(k,v)}
  removeItem(k){this.values.delete(k)}
  get length(){return this.values.size}
  key(i){return [...this.values.keys()][i]??null}
}
const storage=new Local();
const make=()=>createPalWorlds({species,traitData,readPassives:()=>({}),validatePlan:p=>validateBreedingPlan(p,species,traitData),storage,session:new Local(),events:null});
const invFor=w=>createPalInventory({species,traitData,storage:w.storage,events:w.events});
const me='0'.repeat(31)+'1',other='abcdef01'.repeat(4);
const game={id:'pal-'+'a'.repeat(64),name:'测试世界',platform:'Steam'};
const uuids=id=>id.toString(16).padStart(32,'0');
const row=(index,fields={})=>({ownerId:me,instanceId:uuids(index),speciesKey:'SheepBall',name:'小羊',sex:'M',traits:['legend'],...fields});
async function test(){
  const w=make(),inv=invFor(w);
  const manual=inv.add(0,{name:'小羊',sex:'M',traits:['Legend']});
  const world=await w.openGameSave(game,w.active().id);
  const otherWorld=w.create('另一世界');
  const s=(owner,rows,start,before=null)=>w.importTerminal(world.id,game.id,owner,rows,start,before);
  const entries=[row(1),row(2,{name:'  小羊  ',traits:['Unlisted_MOD','Rare'],sex:'F'}),
    row(3,{speciesKey:'bOsS_ShEePBaLL',name:''}),row(4,{speciesKey:'GYM_ElecPanda_Otomo',name:''}),
    row(5,{speciesKey:'Believer_CrossBow',name:'',sex:'unknown'}),row(6,{speciesKey:'NewSpecies',name:''})];
  const initial=await s(me,[...entries,entries[0]],100);
  assert.equal(initial.added,6);
  assert.equal(w.terminalOwner(world.id,game.id),me);
  assert.equal(inv.list().length,0);
  w.select(world.id);
  const record=n=>inv.list().find(p=>p.id.endsWith(uuids(n)));
  assert.equal(inv.list().length,7);
  assert.equal(record(2).name,'  小羊  ');
  assert.deepEqual(record(2).traits,['Unlisted_MOD','Rare']);
  assert.equal(record(3).name,'棉悠悠');
  assert.equal(record(4).name,'佐伊 & 暴电熊');assert.equal(record(4).speciesKey,'ElecPanda');
  assert.equal(record(5).name,'帕鲁保护团体 支持者');assert.equal(record(5).species,-1);
  assert.equal(record(6).name,'NewSpecies');
  const repeated=await s(me,entries,101,me);
  assert.equal(repeated.added,0);assert.equal(repeated.unchanged,6);
  assert.equal(inv.list().length,7);
  inv.update(record(1).id,{name:'手工修改'});assert.equal(record(1).name,'手工修改');
  const changed=await s(me,[row(1,{name:'游戏新名',traits:['Rare'],sex:'F'}),...entries.slice(1)],102,me);
  assert.equal(changed.updated,1);assert.equal(record(1).name,'游戏新名');assert.equal(record(1).sex,'F');
  inv.remove(record(3).id);assert.equal((await s(me,entries,103,me)).ignored,1);
  const plan={id:'plan-kept',targetKey:'SheepBall',traits:['Rare'],random:false,surgery:true,created:1,initialRoute:null,log:[],pending:null};
  w.storage.setItem('pal-atlas.plan.v1.'+plan.id,JSON.stringify(plan));
  const before=w.exportBackup();
  const theirs=[row(9,{ownerId:other,name:'他们的'})];
  await assert.rejects(s(other,[...theirs,row(11,{ownerId:other,sex:'invalid'})],104,me));
  assert.deepEqual(w.exportBackup(),before);assert.equal(w.terminalOwner(world.id,game.id),me);
  storage.reject=k=>k.includes('.terminal.');
  await assert.rejects(s(other,theirs,104,me),/写入/);
  storage.reject=null;
  assert.deepEqual(w.exportBackup(),before);assert.equal(w.terminalOwner(world.id,game.id),me);
  await assert.rejects(s(other,[entries[1]],104,me),/所属/);
  assert((await s(other,theirs,102,me)).stale);
  assert.equal(w.terminalOwner(world.id,game.id),me);
  const switched=await s(other,theirs,104,me);
  assert.equal(switched.archived,5);assert.equal(switched.added,1);
  assert.equal(inv.archived().length,5);assert.deepEqual(inv.list().map(p=>p.name),['小羊','他们的']);
  assert.equal(inv.restore(inv.archived()[0].id),null);
  const b=w.exportBackup();
  assert.deepEqual(b.worlds[0].plans,[plan]);assert(!JSON.stringify(b).includes('terminalStamp'));
  assert.equal(w.importBackup(w.previewImport(b)).length,1);assert(!w.active().gameId);
  assert.deepEqual(inv.list().map(p=>p.name),['小羊','他们的']);assert.equal(inv.archived().length,5);
  assert.equal(w.terminalOwner(w.active().id,game.id),null);
  w.select(world.id);
  const returned=await s(me,entries,105,other);
  assert.equal(returned.restored,5);assert.equal(returned.archived,1);assert.equal(returned.ignored,1);
  assert.equal(inv.list().length,6);assert.deepEqual(inv.archived().map(r=>r.name),['他们的']);
  assert(inv.list().some(p=>p.id===manual.id));
  assert.equal(make().terminalOwner(world.id,game.id),me);
  const otherWindow=make();
  await assert.rejects(otherWindow.importTerminal(world.id,game.id,other,theirs,106,null),/玩家已变化/);
  // Synchronize a reduced terminal and an empty terminal; manual stock survives.
  const reduced=await s(me,[entries[0]],107,me);
  assert.equal(reduced.archived,4);
  assert.deepEqual(inv.list().map(p=>p.id).sort(),[manual.id,record(1).id].sort());
  assert.equal(inv.archived().length,5);
  const empty=await s(me,[],108,me);
  assert.equal(empty.archived,1);
  assert.deepEqual(inv.list().map(p=>p.id),[manual.id]);
  const emptyAgain=await s(me,[],109,me);
  assert.equal(emptyAgain.restored,0);assert.equal(emptyAgain.archived,0);
  assert.equal(inv.list().length,1);
  const reappeared=await s(me,[entries[1]],110,me);
  assert.equal(reappeared.restored,1);assert.equal(inv.list().length,2);
  assert.equal(record(2).name,entries[1].name);
  assert.equal(inv.archived().length,5);
  const snapshot=w.exportBackup();
  await assert.rejects(s(me,[row(20),row(21,{sex:'invalid'})],111,me));
  assert.deepEqual(w.exportBackup(),snapshot);
  assert((await s(me,entries,109,me)).stale);
  assert.deepEqual(w.exportBackup(),snapshot);
  await w.unlinkGame(world.id);
  await assert.rejects(s(me,entries,107,me),/关联已变化/);
  w.select(otherWorld.id);assert.equal(inv.list().length,0);
  console.log('Passed terminal import, physical IDs, localized and unknown species/traits, reruns, edits, deletions, backups, wrong-world isolation, owner archive/restore, quota rollback, stale reads and cross-window choices.');
}
test().catch(e=>{console.error(e);process.exitCode=1});

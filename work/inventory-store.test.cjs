const assert = require('node:assert/strict');
const {createPalInventory} = require('./ui/inventory-store.js');
const {species} = require('./data/breeding.json');
const traitData = require('./data/traits.json');
const key = index => species[index].key;
const legacyStock = 'pal-atlas.warehouse.v1';
const legacyProfiles = 'pal-atlas.trait-profiles.v1';
class MemoryStorage {
  constructor() {this.entries=new Map();}
  get length() {return this.entries.size;}
  key(index) {return [...this.entries.keys()][index] ?? null;}
  getItem(key) {return this.entries.get(key) ?? null;}
  setItem(key,value) {this.entries.set(key,String(value));}
}
const open = (storage,events) => createPalInventory({species,traitData,storage,events});
const storage=new MemoryStorage();
const initialStock=JSON.stringify({owned:[key(0),key(1),key(0)],limit:'3'});
const existingProfiles=JSON.stringify({profiles:[
  {id:'one',speciesKey:key(0),name:'团团',sex:'any',traits:['Legend','Rare']},
  {id:'one-F',speciesKey:key(0),name:'球球',sex:'F',traits:['MoveSpeed_up_3']},
  {id:'asleep',speciesKey:key(2),name:'皮皮',sex:'any',traits:['Rare']},
]});
storage.setItem(legacyStock,initialStock);
storage.setItem(legacyProfiles,existingProfiles);
const a=open(storage);
const check=()=>a.list();
assert.equal(a.getLimit(),'3');
assert.equal(a.persistent,true);
assert.deepEqual(check().map(p=>[p.name,p.sex,p.traits,p.legacy]),[
  ['团团','M',['Legend','Rare'],false],['团团','F',['Legend','Rare'],false],
  ['球球','F',['MoveSpeed_up_3'],false],['个体 1','unknown',[],true]
]);
assert.equal(check().length,new Set(check().map(p=>p.id)).size);
assert(check().some(p=>p.id==='one-F' && p.name==='球球'));
assert(check().every(p=>p.speciesKey===key(p.species)));
assert.deepEqual(a.archived().map(p=>p.sex),['M','F']);
assert.equal(open(storage).list().length,4);
assert.equal(storage.getItem(legacyStock),initialStock);
assert.equal(storage.getItem(legacyProfiles),existingProfiles);
const copy=check()[0];
copy.traits.length=0;
assert.equal(check()[0].traits.length,2);
const updates=[];
const unsubscribe=a.subscribe(()=>updates.push(check().length));
const fresh=a.addMany([1,1,1]);
assert.deepEqual(fresh.map(p=>[p.name,p.sex,p.legacy]),[
  ['个体 2','unknown',false],['个体 3','unknown',false],['个体 4','unknown',false]
]);
assert.deepEqual(updates,[7]);
const b=open(storage);
const fromB=b.add(2,{name:'咕咕',sex:'F',traits:['Legend']});
const fromA=a.add(2,{name:'咯咯',sex:'M',traits:['Rare']});
b.refresh();
assert.deepEqual(a.list(),b.list());
assert.equal(a.list().length,9);
assert.equal(a.archived(2).length,2);
assert.equal(a.restore('asleep').sex,'M');
assert.equal(a.archived().length,1);
assert.equal(a.restore('asleep'),null);
assert.equal(a.setLimit('0'),false);
assert.equal(a.setLimit('all'),true);
assert.equal(open(storage).getLimit(),'all');
assert.throws(()=>a.add(123456),TypeError);
assert.throws(()=>a.add(0,{sex:'any'}),TypeError);
assert.throws(()=>a.update(fresh[0].id,{traits:['not-in-database']}),TypeError);
const changed=b.update(fromA.id,{name:'小咯',traits:['Legend','Legend']});
assert.deepEqual(changed.traits,['Legend']);
a.refresh();
assert.equal(a.list().find(p=>p.id===fromA.id).name,'小咯');
assert(a.remove(fromB.id));
assert.equal(b.update(fromB.id,{name:'stale'}),null);
assert.equal(a.remove(fromB.id),false);
for (const pal of a.list()) assert(a.remove(pal.id));
assert.equal(a.list().length,0);
assert.equal(a.has(0),false);
assert.equal(open(storage).list().length,0);
assert.equal(open(storage).archived().length,1);
assert.equal(a.add(2).name,'个体 1');
assert.equal(a.list().length,1);
unsubscribe();
const count=updates.length;
a.add(1);
assert.equal(updates.length,count);

// Two tabs flush different record changes, including a simultaneous-like
// interleaving between an add's snapshot read and its physical write.
const originalSet=storage.setItem.bind(storage);
let overlap=true;
storage.setItem=(k,v)=>{
  if (overlap && k.startsWith('pal-atlas.inventory.v2.add.')) {
    overlap=false;
    b.add(4,{name:'其他窗口'});
  }
  originalSet(k,v);
};
a.add(3,{name:'当前窗口'});
const afterRace=open(storage);
assert(afterRace.list().some(row=>row.name==='其他窗口'));
assert(afterRace.list().some(row=>row.name==='当前窗口'));

const events=new EventTarget(), live=open(storage,events);
let notices=0;
live.subscribe(()=>notices++);
a.remove(a.list()[0].id);
const storageEvent=new Event('storage');
Object.defineProperty(storageEvent,'key',{value:'pal-atlas.inventory.v2'});
events.dispatchEvent(storageEvent);
assert.deepEqual(live.list(),a.list());
assert.equal(notices,1);
events.dispatchEvent(storageEvent);
assert.equal(notices,1);

const denyWrite=new MemoryStorage();
denyWrite.setItem(legacyStock,initialStock);
denyWrite.setItem(legacyProfiles,existingProfiles);
denyWrite.setItem=()=>{throw new Error('storage blocked');};
const memory=open(denyWrite);
assert.equal(memory.persistent,false);
assert.equal(memory.list().length,4);
const volatile=memory.addMany([1,1]);
assert.equal(memory.list().length,6);
assert.equal(memory.update(volatile[0].id,{name:'临时个体'}).name,'临时个体');
assert(memory.remove(volatile[0].id));
memory.refresh();
assert.equal(memory.list().length,5);
assert.equal(memory.list().some(p=>p.id===volatile[0].id),false);

const privateMode=open(null);
assert.equal(privateMode.persistent,false);
const privatePal=privateMode.add(1);
assert.equal(privateMode.update(privatePal.id,{sex:'M'}).sex,'M');
assert.equal(privateMode.list().length,1);
console.log('Inventory store passed migration, inactive restoration, duplicate individuals, no-ghost removal, batch commit, concurrent tabs and blocked-storage checks.');

const assert = require('node:assert/strict');
const {performance} = require('node:perf_hooks');
const {solve} = require('./ui/trait-solver.js');
const db = require('./data/breeding.json');

const pal = (id,species,traits=[],sex='any') => ({id,species,traits,sex});

// Independently enumerate individual states, freezing a generation before adding babies.
// States with trailing unwanted traits can also serve any requested subset of their genes.
function oracle({pairs,speciesCount,stock,traits,allowRandom=false,randomEligible=[]}) {
  const size = 1 << traits.length;
  const depth = Array.from({length:speciesCount}, () => Array.from({length:2},() => Array(size).fill(Infinity)));
  function insert(p,s,mask,n) {
    let changed = false;
    for (let sub=0; sub<size; sub++) if ((sub & mask) === sub && n < depth[p][s][sub]) {
      depth[p][s][sub]=n; changed=true;
    }
    return changed;
  }
  for (const item of stock) {
    const mask = traits.reduce((m,t,i)=>m|(item.traits.includes(t)?1<<i:0),0);
    if (item.sex !== 'F') insert(item.species,0,mask,0);
    if (item.sex !== 'M') insert(item.species,1,mask,0);
  }
  for (let gen=1; ;gen++) {
    const babies=[];
    for (const [a,b,c,s] of pairs) for (let sex=0;sex<2;sex++) {
      if (s && s !== ['M','F'][sex]) continue;
      for (let i=0;i<size;i++) for (let j=0;j<size;j++) {
        if (depth[a][sex][i]>=gen || depth[b][1-sex][j]>=gen) continue;
        babies.push([c,i|j]);
        if (allowRandom) traits.forEach((t,k)=>{if (randomEligible.includes(t)) babies.push([c,i|j|(1<<k)]);});
      }
    }
    let added=false;
    for (const [c,mask] of babies) for (let s=0;s<2;s++) if (insert(c,s,mask,gen)) added=true;
    if (!added) return depth;
  }
}

function replay(input, result) {
  const {pairs,stock,target,traits} = input;
  const nodes = new Map();
  for (const source of result.founders) {
    const record=stock.find(r=>r.id===source.stockId);
    assert(record && record.species===source.species);
    assert.deepEqual(record.traits,source.traits);
    for (const t of source.requiredTraits) assert(record.traits.includes(t));
    nodes.set(source.id,{...record,depth:0});
  }
  for (const step of result.steps) {
    assert(!nodes.has(step.id));
    const [a,b]=step.parents;
    assert.notEqual(a.sex,b.sex);
    assert(pairs.some(([x,y,c,s])=>c===step.species && x===a.species && y===b.species && (!s || s===a.sex)));
    for (const parent of [a,b]) {
      const node=nodes.get(parent.id);
      assert(node && node.species===parent.species);
      assert(!['M','F'].includes(node.sex) || node.sex === parent.sex);
      for (const t of parent.traits) assert(node.traits.includes(t));
    }
    assert.equal(step.depth,1+Math.max(nodes.get(a.id).depth,nodes.get(b.id).depth));
    assert.equal(step.randomTraits.length<=1,true);
    for (const t of step.randomTraits) assert(input.allowRandom && input.randomEligible.includes(t));
    assert.deepEqual([...new Set([...a.traits,...b.traits,...step.randomTraits])].sort(),[...step.traits].sort());
    nodes.set(step.id,step);
  }
  const end=nodes.get(result.targetId);
  assert.equal(end.species,target);
  assert.equal(end.depth,result.generations);
  for (const t of traits) assert(end.traits.includes(t));
}
const fixture = {speciesCount:5,pairs:[[0,1,2],[0,2,3],[1,2,1],[2,3,4,'F','M'],[3,3,0]]};
const stock=[pal('a',0,['swift','lucky'],'M'),pal('b',1,['work'],'F'),pal('c',0,['legend'],'M')];
const goals=['swift','lucky','work','legend'];
for (const allowRandom of [false,true]) {
  for (const requested of [goals,[],['swift','work'],['swift','legend','random1','random2']]) {
    const input={...fixture,stock,traits:requested,allowRandom,randomEligible:['random1','random2','work']};
    const depths=oracle(input);
    for (let target=0;target<fixture.speciesCount;target++) {
      const result=solve({...input,target});
      const mask=(1<<requested.length)-1;
      const expected=Math.min(depths[target][0][mask],depths[target][1][mask]);
      if (!Number.isFinite(expected)) assert(!['owned','ok'].includes(result.status));
      else {
        assert.equal(result.generations,expected);
        replay({...input,target},result);
      }
    }
  }
}
const self={speciesCount:1,pairs:[[0,0,0]],target:0,traits:['a','b']};
assert.equal(solve({...self,stock:[pal('male1',0,['a'],'M'),pal('male2',0,['b'],'M')]}).status,'unreachable-traits');
assert.equal(solve({...self,stock:[pal('male1',0,['a'],'M'),pal('female2',0,['b'],'F')]}).generations,1);
assert.equal(solve({...self,stock:[pal('bothtraits',0,['a','b'],'M')]}).status,'owned');
assert.equal(solve({...self,stock:[pal('short',0,['a'])]}).status,'missing-traits');
const random={...self,stock:[pal('wild',0,[])],traits:['a','b','c','d'],allowRandom:true,randomEligible:['a','b','c','d']};
const chance=solve(random);
assert.equal(chance.generations,3);
replay(random,chance);
assert.equal(solve({...random,randomEligible:['a','b','c']}).status,'missing-traits');
assert.equal(solve({...self,traits:['a','b','c','d','e'],stock:[]}).status,'invalid');
const inaccessible = {speciesCount:2,pairs:[[0,0,0]],target:0,traits:['a'],stock:[pal('breed',0,[]),pal('locked',1,['a'])]};
assert.equal(solve(inaccessible).status,'unreachable-traits');
assert.equal(solve({...inaccessible,allowRandom:true,randomEligible:['a']}).generations,1);

const id = name => db.species.findIndex(p=>p.en===name);
const p = (name,traits=[],sex='any',n='') => pal(name+n,id(name),traits,sex);
const full = {pairs:db.pairs,speciesCount:db.species.length};
const kat = p('Katress',['x'],'F'), fox=p('Wixen',['y'],'M');
const test = {...full,stock:[kat,fox],target:id('Katress Ignis'),traits:['x','y']};
const sexResult=solve(test);
assert.equal(sexResult.generations,1); replay(test,sexResult);
assert.notEqual(solve({...test,target:id('Wixen Noct')}).generations,1);
assert.equal(solve({...full,stock:[p('Lamball',['x'],'M'),p('Cattiva',['y'],'M')],target:id('Daedream'),traits:['x','y']}).status,'unreachable-species');

const benchmarks = [
  {...full,stock:[p('Lamball',['a','b']),p('Cattiva',['c','d'])],target:id('Menasting'),traits:['a','b','c','d']},
  {...full,stock:db.species.map((s,i)=>pal(s.key,i,i<4?[['a','b','c','d'][i]]:[],i%2?'M':'F')),target:id('Anubis'),traits:['a','b','c','d']},
  {...full,stock:[p('Lamball',[]),p('Cattiva',[])],target:id('Jetragon'),traits:['a','b','c','d'],allowRandom:true,randomEligible:['a','b','c','d']}
];
for (const testCase of benchmarks) {
  const start = performance.now(), result=solve(testCase);
  if (['ok','owned'].includes(result.status)) replay(testCase,result);
  console.log('Benchmark',testCase.stock.length,db.species[testCase.target].en,result.status,result.generations,'in',Math.round(performance.now()-start),'ms');
}
console.log('Trait solver: reference minima, witnessed routes and gender, same-species profiles, optional random inheritance, and 4-slot checks passed.');

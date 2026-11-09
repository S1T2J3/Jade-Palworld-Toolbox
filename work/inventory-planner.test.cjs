const assert = require('node:assert/strict');
const {solve} = require('./ui/trait-solver.js');
const {species,pairs} = require('./data/breeding.json');
const make = (id,species,sex='unknown',traits=[])=>({id,species,sex,traits});
const get = (rows,count,stock,target,traits=[],requireHatch=false)=>solve({pairs:rows,speciesCount:count,stock,target,traits,requireHatch});
const minimal = [[0,0,1]];
const one = [make('one',0)];
assert.equal(get(minimal,2,one,0).status,'owned');
assert.equal(get(minimal,2,one,1).status,'unreachable-species');
assert.equal(get(minimal,2,[...one,make('two',0)],1).status,'ok');
assert.equal(get([[0,0,0]],1,one,0,[],true).status,'unreachable-species');
const two = get([[0,0,0]],1,[...one,make('two',0)],0,[],true);
assert.equal(two.steps.length,1);
assert.deepEqual(two.founders.map(f=>f.assumedSex).sort(),['F','M']);
assert.equal(two.founders[0].stockId===two.founders[1].stockId,false);
assert.equal(two.steps[0].species,0);
const fork = [[0,1,2,'M','F'],[0,1,3,'F','M'],[2,3,4]];
const ambiguous=[make('a',0),make('b',1)];
assert.equal(get(fork,5,ambiguous,4).status,'unreachable-species','A founder cannot change sex between branches');
const forkRoute=get(fork,5,[...ambiguous,make('aa',0),make('bb',1)],4);
assert.equal(forkRoute.generations,2);
assert.equal(forkRoute.founders.length,4);
assert(forkRoute.founders.every(f=>f.sexes.length===1));

// Full independent binary assignment enumeration on deterministic small graphs.
let state=73621;
function random(n) {state=(state*48271)%2147483647;return state%n;}
for (let trial=0;trial<120;trial++) {
  const count=6;
  const rows=Array.from({length:14},()=>{
    const a=random(count),b=random(count),c=random(count);
    return random(4)===0?[a,b,c,'F','M']:[a,b,c];
  });
  const founders=Array.from({length:4},(_,i)=>make(String(i),random(count),i===3?'M':'unknown',random(2)?['swift']:[]));
  const target=random(count), wanted=random(2)?['swift']:[];
  for (const hatch of [true,false]) {
    let min=Infinity;
    for (let bits=0;bits<8;bits++) {
      const fixed=founders.map((f,i)=>f.sex==='unknown'?{...f,sex:(bits&(1<<i))?'M':'F'}:f);
      const r=get(rows,count,fixed,target,wanted,hatch);
      if (r.status==='ok'||r.status==='owned') min=Math.min(min,r.generations);
    }
    const result=get(rows,count,founders,target,wanted,hatch);
    assert.equal(result.generations ?? Infinity,min,JSON.stringify({rows,founders,target,wanted,hatch}));
    for (const f of result.founders) assert(f.sexes.length<=1);
  }
}
const sample=get(pairs,species.length,[make('lamb',0),make('cat',1)],4);
assert.equal(sample.generations,2);
assert(sample.conditionalSex);
console.log('Unknown individual sex assignments, re-hatching and minimum depth match exhaustive assignment on 240 small cases.');

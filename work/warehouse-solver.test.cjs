const assert = require('node:assert/strict');
const {solve, route} = require('./ui/warehouse-solver.js');
const {species, pairs} = require('./data/breeding.json');

// Branches share the intermediate at index 2. Two final dependencies that
// can be raised in parallel count as one generation, with four actual recipes.
const fork = [[0,1,2],[0,2,3],[1,2,4],[3,4,5],[5,6,7],[7,0,6]];
const direct = solve(fork, 8, [0, 1, 99, -1, 1], 1);
assert.deepEqual(direct.depth, [0,0,1,Infinity,Infinity,Infinity,Infinity,Infinity]);
const all = solve(fork, 8, [0,1]);
assert.deepEqual(all.depth, [0,0,1,2,2,3,Infinity,Infinity]);
assert.deepEqual(route(all,5), fork.slice(0,4));
assert.deepEqual(route(all,6), []);
assert.deepEqual(route(all,0), []);
assert.deepEqual(solve(fork, 8, [], Infinity).depth, Array(8).fill(Infinity));
assert.deepEqual(solve(fork, 8, [1], 0).depth, [Infinity,0,Infinity,Infinity,Infinity,Infinity,Infinity,Infinity]);
assert.deepEqual(solve([...fork].reverse(),8,[0,1]).depth, all.depth);

const reproducing = [[0,1,2], [2,1,0,'F','M'], [0,2,1], [3,3,3]];
const ownedRoute = solve(reproducing,5,[0,1,3,4]);
assert.deepEqual(ownedRoute.bredDepth, [2,2,1,1,Infinity]);
assert.deepEqual(ownedRoute.depth, [0,0,1,0,0]);
assert.deepEqual(ownedRoute.recipe[0], null);
assert.deepEqual(route(ownedRoute,0,true), reproducing.slice(0,2));
assert.deepEqual(route(ownedRoute,0), []);
assert.deepEqual(route(ownedRoute,3,true), [reproducing[3]]);
assert.deepEqual(route(ownedRoute,4,true), []);
assert.deepEqual(route(ownedRoute,2,true), route(ownedRoute,2));
assert.deepEqual(solve(reproducing,5,[0,1,3,4],1).bredDepth, [Infinity,Infinity,1,1,Infinity]);
assert.deepEqual(solve(reproducing,5,[0,1,3,4],0).bredDepth, Array(5).fill(Infinity));
assert.deepEqual(solve(reproducing,5,[]).bredDepth, Array(5).fill(Infinity));

const differentRoutes = [[0,1,2],[0,1,3],[2,3,4],[0,3,4]];
assert.deepEqual(solve(differentRoutes,5,[0,1]).recipe[4], [0,3,4]);

const id = name => {
  const index = species.findIndex(pal => pal.en === name);
  assert.ok(index >= 0, name);
  return index;
};
const lamball = id('Lamball'), cattiva = id('Cattiva'), daedream = id('Daedream');
const starter = solve(pairs,species.length,[lamball,cattiva]);
assert.equal(starter.depth[daedream], 1);
assert.equal(starter.depth[id('Astralym')], Infinity);
assert.equal(starter.depth[id('Panthalus')], Infinity);
assert.equal(solve(pairs,species.length,[lamball]).depth.filter(Number.isFinite).length, 1);
const single = solve(pairs,species.length,[lamball]);
assert.equal(single.bredDepth[lamball], 1);
assert.deepEqual(route(single,lamball,true), [[lamball,lamball,lamball]]);
const blocked = id('Panthalus');
const onlyBlocked = solve(pairs,species.length,[blocked]);
assert.equal(onlyBlocked.depth[blocked], 0);
assert.equal(onlyBlocked.bredDepth[blocked], Infinity);
assert.equal(onlyBlocked.bredRecipe[blocked], null);
assert.deepEqual(route(onlyBlocked,blocked,true), []);
const wixen = id('Wixen'), katress = id('Katress');
const sexPlan = solve(pairs,species.length,[wixen,katress],1);
for (const [child,firstSex,secondSex] of [['Wixen Noct','F','M'],['Katress Ignis','M','F']]) {
  assert.equal(sexPlan.depth[id(child)], 1);
  assert.deepEqual(route(sexPlan,id(child)), [[wixen,katress,id(child),firstSex,secondSex]]);
}

// A second calculation enumerates only matings of already obtained species.
// Compare all target minima, then replay every resulting route from inventory.
function bruteDepth(owned) {
  const byParents = new Map();
  for (const row of pairs) {
    const key = row[0]*species.length + row[1];
    if (!byParents.has(key)) byParents.set(key,[]);
    byParents.get(key).push(row[2]);
  }
  const result = Array(species.length).fill(Infinity);
  const hatched = Array(species.length).fill(Infinity);
  for (const i of owned) result[i] = 0;
  const known = new Set(owned);
  for (let step = 1; ; step++) {
    const children = new Set();
    for (const a of known) for (const b of known) {
      if (a > b) continue;
      for (const child of byParents.get(a*species.length+b) || []) {
        if (hatched[child] === Infinity) hatched[child] = step;
        if (!known.has(child)) children.add(child);
      }
    }
    if (!children.size) return {result, hatched};
    for (const child of children) { result[child] = step; known.add(child); }
  }
}
for (const owned of [[lamball,cattiva],[wixen,katress],[id('Jetragon'),id('Chikipi'),id('Gumoss (Flower)')]]) {
  const plan = solve(pairs,species.length,owned);
  const {result, hatched} = bruteDepth(owned);
  assert.deepEqual(plan.depth, result);
  assert.deepEqual(plan.bredDepth, hatched);
  for (let target = 0; target < species.length; target++) {
    const steps = route(plan,target);
    const available = new Set(owned);
    let previousStep = 0;
    for (const [a,b,c] of steps) {
      assert.ok(available.has(a) && available.has(b), 'parent not yet available');
      assert.ok(!available.has(c), 'intermediate repeated');
      assert.equal(plan.depth[c], 1+Math.max(plan.depth[a],plan.depth[b]));
      assert.ok(plan.depth[c] >= previousStep, 'route generations out of order');
      previousStep = plan.depth[c];
      available.add(c);
    }
    assert.equal(available.has(target), Number.isFinite(plan.depth[target]));
    if (owned.includes(target)) {
      const row = plan.bredRecipe[target];
      assert.ok(row && owned.includes(row[0]) && owned.includes(row[1]));
      assert.deepEqual(route(plan,target,true), [row]);
    } else {
      assert.equal(plan.bredRecipe[target], plan.recipe[target]);
      assert.deepEqual(route(plan,target,true), steps);
    }
  }
}
console.log('Warehouse solver passed generation limits, shared routes, gender rules, and full-dataset differential tests.');

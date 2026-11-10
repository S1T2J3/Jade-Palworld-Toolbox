const TraitSolver = (() => {
  const sexes = ['M', 'F'];

  // At equal breeding depth, save the most operations, then consumables, then coins.
  function compareRoutes(a,b) {
    const left=a.surgery||[], right=b.surgery||[];
    return a.generations-b.generations || left.length-right.length ||
      left.filter(op=>op.mode==='consume').length-right.filter(op=>op.mode==='consume').length ||
      left.reduce((sum,op)=>sum+op.cost,0)-right.reduce((sum,op)=>sum+op.cost,0);
  }
  function solveFixed({pairs, speciesCount, stock, target, traits = [], allowRandom = false, randomEligible = [], surgeryOptions = []}) {
    const failure = (status, missing = []) => ({status, missing, generations:null, steps:[], founders:[], targetId:null});
    if (!Array.isArray(pairs) || !Array.isArray(stock) || !Array.isArray(traits) ||
        !Number.isInteger(speciesCount) || !Number.isInteger(target) || target < 0 || target >= speciesCount) {
      return failure('invalid');
    }
    const wanted = [...new Set(traits)];
    if (wanted.length > 4 || wanted.some(key => typeof key !== 'string' || !key)) return failure('invalid');
    const records = stock.filter(item => item && Number.isInteger(item.species) &&
      item.species >= 0 && item.species < speciesCount && Array.isArray(item.traits));
    if (!records.length) return failure('unreachable-species');

    const maskCount = 1 << wanted.length;
    const allTraits = maskCount - 1;
    const allSets = (1 << maskCount) - 1;
    const strain = (species, sex) => species * 2 + sex;
    const slot = (strainId, mask) => strainId * maskCount + mask;
    const traitKeys = mask => wanted.filter((key,i) => mask & (1 << i));
    const keyBits = new Map(wanted.map((key,i) => [key, 1 << i]));
    const traitMask = keys => keys.reduce((mask,key) => mask | (keyBits.get(key) || 0), 0);
    const operations=surgeryOptions.filter(op=>op && keyBits.has(op.key));
    const inheritedMask=allTraits & ~traitMask(operations.map(op=>op.key));
    // Each bit in this word represents ONE attainable trait subset on ONE individual.
    // Subsets from different individuals only combine in a legal mating.
    const subsets = Array.from({length:maskCount}, (_, mask) => {
      let bits = 0;
      for (let i = 0; i < maskCount; i++) if ((mask & i) === i) bits |= 1 << i;
      return bits;
    });
    let available = new Uint16Array(speciesCount * 2);
    const level = new Int32Array(available.length * maskCount).fill(-1);
    const founderOf = new Int32Array(level.length).fill(-1);
    let donorMask = 0;
    records.forEach((record, index) => {
      const mask = traitMask(record.traits);
      donorMask |= mask;
      for (let sex = 0; sex < 2; sex++) {
        if (sexes.includes(record.sex) && record.sex !== sexes[sex]) continue;
        const s = strain(record.species,sex);
        available[s] |= subsets[mask];
        for (let m = 0; m <= allTraits; m++) {
          if ((mask & m) !== m) continue;
          const position = slot(s,m);
          if (founderOf[position] >= 0) continue;
          level[position] = 0;
          founderOf[position] = index;
        }
      }
    });
    const absent = allTraits & ~donorMask;
    // Available surgery supplies these traits after breeding, so do not require
    // a random mutation for the same trait even when random hatching is enabled.
    const randomMask = allowRandom ? traitMask(randomEligible) & inheritedMask : 0;
    const missing = traitKeys(absent & inheritedMask & ~randomMask);
    if (missing.length) return failure('missing-traits', missing);

    const unionMemo = new Map();
    function combine(left, right) {
      if (!left || !right) return 0;
      if (left === 1) return right;
      if (right === 1 || left === allSets) return left;
      if (right === allSets) return right;
      const key = Math.min(left,right) * 65536 + Math.max(left,right);
      const cached = unionMemo.get(key);
      if (cached !== undefined) return cached;
      let result = 0;
      for (let a = 0; a <= allTraits; a++) {
        if (!(left & (1 << a))) continue;
        for (let b = 0; b <= allTraits; b++) {
          if (right & (1 << b)) result |= 1 << (a | b);
        }
      }
      unionMemo.set(key,result);
      return result;
    }
    const randomMemo = new Map();
    function withRandom(sets) {
      if (!randomMask || !sets || sets === allSets) return sets;
      const cached = randomMemo.get(sets);
      if (cached !== undefined) return cached;
      let result = sets;
      for (let mask = 0; mask <= allTraits; mask++) {
        if (!(sets & (1 << mask))) continue;
        for (let bit = 1; bit <= randomMask; bit *= 2) {
          if (randomMask & bit) result |= 1 << (mask | bit);
        }
      }
      randomMemo.set(sets,result);
      return result;
    }

    const goalStrain = strain(target,0);
    const goalReached = () => ((available[goalStrain] | available[goalStrain + 1]) & (1 << inheritedMask)) !== 0;
    const recipesByChild = Array.from({length:speciesCount}, () => []);
    // Row[3], when present, fixes which sex the first species must supply.
    // All breedable species in this dataset can hatch as either sex.
    for (const row of pairs) recipesByChild[row[2]].push(row);
    for (let generation = 1; !goalReached(); generation++) {
      const next = available.slice();
      let added = false;
      for (const [a,b,c,requiredSex] of pairs) {
        const c0 = strain(c,0);
        if ((next[c0] & next[c0+1]) === allSets) continue;
        const a0 = strain(a,0), b0 = strain(b,0);
        let outcomes = 0;
        if (requiredSex !== 'F') outcomes = combine(available[a0],available[b0+1]);
        if (requiredSex !== 'M') outcomes |= combine(available[a0+1],available[b0]);
        outcomes = withRandom(outcomes);
        if (!outcomes) continue;
        for (let s = c0; s <= c0+1; s++) {
          let newSets = outcomes & ~next[s];
          if (!newSets) continue;
          added = true;
          next[s] |= newSets;
          while (newSets) {
            const bit = newSets & -newSets;
            const mask = 31 - Math.clz32(bit);
            level[slot(s,mask)] = generation;
            newSets ^= bit;
          }
        }
      }
      if (!added) {
        const speciesReachable = (available[goalStrain] | available[goalStrain+1]) & 1;
        return failure(speciesReachable ? 'unreachable-traits' : 'unreachable-species');
      }
      available = next;
    }

    // Reconstruct a witness on strictly earlier generations. A partition assigns
    // each inherited goal trait to the actual parent that must supply it.
    function parentsFor(child, mask, generation) {
      let best = null, bestPenalty = Infinity;
      for (const row of recipesByChild[child]) {
        const [a,b,,requiredSex] = row;
        for (let sex = 0; sex < 2; sex++) {
          if (requiredSex && sexes[sex] !== requiredSex) continue;
          const sa = strain(a,sex), sb = strain(b,1-sex);
          if (level[slot(sa,0)] < 0 || level[slot(sb,0)] < 0) continue;
          for (let omitted = 0; omitted <= mask; omitted = omitted ? omitted * 2 : 1) {
            if (omitted && !(mask & randomMask & omitted)) continue;
            const inherited = mask ^ omitted;
            for (let left = inherited; ; left = (left - 1) & inherited) {
              const right = inherited ^ left;
              const da = level[slot(sa,left)], db = level[slot(sb,right)];
              if (da >= 0 && db >= 0 && Math.max(da,db) + 1 === generation) {
                // Prefer inheritance on this generation and a shallower second branch.
                const penalty = (omitted ? level.length : 0) + da + db;
                if (penalty < bestPenalty) {
                  best = {a,b,sex,left,right,omitted};
                  bestPenalty = penalty;
                  if (penalty === generation - 1) return best;
                }
              }
              if (left === 0) break;
            }
          }
        }
      }
      if (!best) throw new Error('Missing parent witness for a reachable breeding state');
      return best;
    }
    const founders = new Map();
    const bred = new Map();
    function visit(species, mask, sex, requestedSex) {
      const position = slot(strain(species,sex),mask);
      const depth = level[position];
      if (depth === 0) {
        const index = founderOf[position];
        const actual = records[index];
        if (!founders.has(index)) {
          founders.set(index,{id:'f'+index,stockId:actual.id,species,traits:actual.traits.slice(),sex:actual.sex || 'any',
            requiredMask:0,sexes:new Set()});
        }
        const founder = founders.get(index);
        founder.requiredMask |= mask;
        if (requestedSex) founder.sexes.add(requestedSex);
        return founder.id;
      }
      const key = species * maskCount + mask;
      if (!bred.has(key)) {
        const witness = parentsFor(species,mask,depth);
        const {a,b,left,right,omitted} = witness;
        const maleOrFemale = witness.sex;
        const parents = [[a,left,maleOrFemale],[b,right,1-maleOrFemale]].map(([p,m,s]) => ({
          id:visit(p,m,s,sexes[s]), species:p, traits:traitKeys(m), sex:sexes[s]
        }));
        bred.set(key,{id:'p'+key,species,traits:traitKeys(mask),depth,parents,
          randomTraits:traitKeys(omitted),sexes:new Set()});
      }
      const node = bred.get(key);
      if (requestedSex) node.sexes.add(requestedSex);
      return node.id;
    }
    let bestGoal;
    for (let mask=inheritedMask; mask<=allTraits; mask++) {
      if ((mask & inheritedMask)!==inheritedMask) continue;
      const surgery=operations.filter(op=>!(mask & keyBits.get(op.key)));
      for (let sex=0; sex<2; sex++) {
        const generations=level[slot(goalStrain+sex,mask)];
        if (generations<0) continue;
        const candidate={mask,sex,generations,surgery};
        if (!bestGoal || compareRoutes(candidate,bestGoal)<0) bestGoal=candidate;
      }
    }
    const targetId = visit(target,bestGoal.mask,bestGoal.sex,null);
    const steps = [...bred.values()].sort((a,b) => a.depth-b.depth)
      .map(node => ({...node,sexes:[...node.sexes]}));
    const sources = Array.from(founders.values(), ({requiredMask,sexes,...node}) => ({
      ...node,requiredTraits:traitKeys(requiredMask),sexes:[...sexes]
    }));
    return {status:steps.length ? 'ok' : 'owned', missing:[], generations:bestGoal.generations,surgery:bestGoal.surgery,
      steps, founders:sources, targetId, conditional:steps.some(step => step.randomTraits.length)};
  }
  function solveAssigned(request) {
    if (!Array.isArray(request.stock) || !request.stock.some(row => row?.sex === 'unknown')) return solveFixed(request);
    const known = request.stock.filter(row => row && row.sex !== 'unknown');
    const originals = new Map(request.stock.filter(Boolean).map(row => [row.id, row]));
    const stock = [...known, ...request.stock.filter(row => row?.sex === 'unknown')];
    const wanted = new Set(request.traits || []);
    const groups = new Map();
    for (let i=0; i<stock.length; i++) {
      const row = stock[i];
      const relevant = (row.traits || []).filter(key=>wanted.has(key)).sort();
      const key = JSON.stringify([row.species,relevant]);
      if (!groups.has(key)) groups.set(key,{known:new Set(),unknown:[]});
      const group=groups.get(key);
      if (row.sex==='unknown') group.unknown.push(i);
      else group.known.add(row.sex);
    }
    // Two equivalent specimens can supply one of each sex. For a single
    // equivalent unknown and a known parent the opposite sex covers all
    // combinations the pair could supply under any assignment.
    for (const {known,unknown} of groups.values()) {
      if (!unknown.length || (unknown.length===1 && !known.size)) continue;
      const first=known.has('M')?'F':'M';
      unknown.forEach((i,n)=>{stock[i]={...stock[i],sex:n%2===0?first:(first==='M'?'F':'M')};});
    }

    let solved=0, incomplete=false, best=null;
    const maxSearches=48;
    // In the relaxed search an unassigned animal is visible to either sex.
    // A witness is physically realizable exactly when each unknown founder
    // is used with one consistent sex across all branches.
    function search(candidates) {
      if (solved>=maxSearches) {incomplete=true;return null;}
      solved++;
      const result=solveFixed({...request,stock:candidates});
      if (result.status!=='ok' && result.status!=='owned') return result;
      if (best && compareRoutes(best,result)<=0) return best;
      const conflict=result.founders.find(f=>originals.get(f.stockId)?.sex==='unknown' && f.sexes.length>1);
      if (!conflict) {
        for (const f of result.founders) {
          if (originals.get(f.stockId)?.sex==='unknown') {
            f.sex='unknown';
            f.assumedSex=f.sexes[0] || null;
          }
        }
        result.conditionalSex=result.founders.some(f=>f.assumedSex);
        best=result;
        return result;
      }
      const index=candidates.findIndex(row=>row.id===conflict.stockId);
      let failure=null;
      for (const sex of sexes) {
        const branch=candidates.slice();
        branch[index]={...branch[index],sex};
        const answer=search(branch);
        if (best && compareRoutes(best,result)<=0) return best;
        if (answer && (!failure || answer.status==='unreachable-traits')) failure=answer;
      }
      return best || failure;
    }
    const result=search(stock);
    if (incomplete) {
      // A known-sex route remains executable even when the assignment search
      // reaches its limit. Never label an inconclusive search "unreachable".
      const confirmed=solveFixed({...request,stock:known});
      if ((confirmed.status==='owned' || confirmed.status==='ok') && (!best || compareRoutes(confirmed,best)<=0)) best=confirmed;
      if (best) return {...best,minimumUnconfirmed:true};
      return {status:'unconfirmed-sex',missing:[],generations:null,steps:[],founders:[],targetId:null};
    }
    if (result?.conditionalSex && known.length) {
      const confirmed=solveFixed({...request,stock:known});
      if ((confirmed.status==='owned' || confirmed.status==='ok') && compareRoutes(confirmed,result)<=0) return confirmed;
    }
    return result;
  }
  function solve(request) {
    if (!request.requireHatch) return solveAssigned(request);
    const {speciesCount,target,pairs}=request;
    if (!Array.isArray(pairs) || !Number.isInteger(target) || target<0 || target>=speciesCount) return solveAssigned(request);
    // A separate terminal species prevents the initial inventory from satisfying
    // the goal at depth zero while leaving the real species available as a parent.
    const terminalPairs=pairs.filter(row=>row[2]===target)
      .map(([a,b,,...genders])=>[a,b,speciesCount,...genders]);
    const result=solveAssigned({...request,pairs:pairs.concat(terminalPairs),speciesCount:speciesCount+1,target:speciesCount});
    for (const step of result.steps) if (step.species===speciesCount) step.species=target;
    return result;
  }
  return {solve};
})();

if (typeof globalThis !== 'undefined') globalThis.TraitSolver = TraitSolver;
if (typeof module !== 'undefined' && module.exports) module.exports = TraitSolver;

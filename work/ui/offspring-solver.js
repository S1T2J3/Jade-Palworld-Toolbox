const OffspringSolver = (() => {
  const opposite = sex => sex === 'M' ? 'F' : 'M';

  function parentGroups(stock) {
    const groups = new Map();
    for (const record of stock) {
      if (!groups.has(record.species)) groups.set(record.species,{M:null,F:null,unknown:[]});
      const group = groups.get(record.species);
      if (record.sex === 'M' || record.sex === 'F') group[record.sex] ??= record;
      else group.unknown.push(record);
    }
    for (const group of groups.values()) {
      // Two distinct unknown individuals can supply opposite sexes. With only
      // one, its sex must be consistent everywhere it is used in a given route.
      if (!group.M && !group.F && group.unknown.length === 1) group.singleUnknown = group.unknown[0];
      else {
        if (!group.M) group.M=group.unknown.shift();
        if (!group.F) group.F=group.unknown.shift();
      }
    }
    return groups;
  }

  // A species-only shortest route is already optimal if all of its founder
  // pairings can be assigned consistent sexes. Each mating introduces at most
  // one opposite-sex constraint. A bred intermediate can be hatched repeatedly.
  function validateRoute(route, structural, groups) {
    const assigned = new Map(), edges = new Map();
    const limited = index => {
      const group=groups.get(index);
      return group && !(group.M && group.F);
    };
    function pin(index,sex) {
      if (!limited(index)) return true;
      if (assigned.has(index)) return assigned.get(index) === sex;
      assigned.set(index,sex);
      return true;
    }
    function connect(a,b) {
      if (!edges.has(a)) edges.set(a,new Set());
      edges.get(a).add(b);
    }
    for (const [a,b,,aSex,bSex] of route) {
      for (const id of [a,b]) {
        const group=groups.get(id);
        if (limited(id) && !group.singleUnknown && !pin(id,group.M ? 'M' : 'F')) return null;
      }
      if (aSex) {
        if (!pin(a,aSex) || !pin(b,bSex)) return null;
      } else if (limited(a) && limited(b)) {
        if (a === b) return null;
        connect(a,b);connect(b,a);
      }
    }
    function propagate(seeds) {
      const queue=[...seeds];
      for (let pos=0;pos<queue.length;pos++) {
        const id=queue[pos], needed=opposite(assigned.get(id));
        for (const neighbor of edges.get(id) || []) {
          if (assigned.has(neighbor)) {
            if (assigned.get(neighbor)!==needed) return false;
          } else {assigned.set(neighbor,needed);queue.push(neighbor);}
        }
      }
      return true;
    }
    if (!propagate(assigned.keys())) return null;
    for (const id of edges.keys()) {
      if (!assigned.has(id)) {assigned.set(id,'M');if (!propagate([id])) return null;}
    }
    for (const [a,b] of route) for (const id of [a,b]) {
      if (limited(id) && !assigned.has(id)) assigned.set(id,'M');
    }
    const parents = (index,sex) => {
      const group=groups.get(index), record=group && (group.singleUnknown || group[sex]);
      return {species:index,sex,stockId:record?.id || null,unknown:record?.sex === 'unknown'};
    };
    return route.map(([a,b,child,aSex])=>{
      const first=aSex || assigned.get(a) || (assigned.has(b) ? opposite(assigned.get(b)) : 'M');
      return {child,depth:structural.bredDepth[child],parents:[parents(a,first),parents(b,opposite(first))]};
    });
  }

  // For a concrete assignment every hatched species can provide either sex
  // in later generations. Track sex-specific availability separately from
  // earliest actual hatches: a founder is available at depth zero.
  function fixedAssignment(pairs,speciesCount,parents,items) {
    const depths=[new Int16Array(speciesCount).fill(-1),new Int16Array(speciesCount).fill(-1)];
    for (let sex=0;sex<2;sex++) for (let i=0;i<speciesCount;i++) if (parents[sex][i]) depths[sex][i]=0;
    const hatch=new Int16Array(speciesCount).fill(-1);
    const secondary=new Int16Array(speciesCount);
    const witness=new Array(speciesCount);
    for (let generation=1;;generation++) {
      let added=false;
      for (const [a,b,c,requiredSex] of pairs) {
        if (hatch[c]>=0 && hatch[c]<generation) continue;
        for (let first=0;first<2;first++) {
          if (requiredSex && requiredSex!==(first===0?'M':'F')) continue;
          const da=depths[first][a], db=depths[1-first][b];
          if (da<0||db<0||Math.max(da,db)!==generation-1) continue;
          const score=Math.min(da,db);
          if (hatch[c]===generation && secondary[c]<=score) continue;
          hatch[c]=generation;
          secondary[c]=score;
          witness[c]=[a,b,first];
          // A same-round child receives this generation, so it cannot qualify
          // as a parent until the next iteration.
          for (const table of depths) if (table[c]<0) {table[c]=generation;added=true;}
        }
      }
      if (!added) break;
    }
    for (let target=0;target<speciesCount;target++) {
      if (hatch[target]<0 || (items[target] && items[target].depth<=hatch[target])) continue;
      const steps=[],seen=new Set();
      function visit(c) {
        if (seen.has(c)) return;
        seen.add(c);
        const [a,b,first]=witness[c];
        const pair=[a,b].map((index,k)=>{
          const s=k===0?first:1-first;
          const record=parents[s][index];
          if (!record) visit(index);
          return {species:index,sex:s===0?'M':'F',stockId:record?.id||null,unknown:record?.sex==='unknown'};
        });
        steps.push({child:c,depth:hatch[c],parents:pair});
      }
      visit(target);
      steps.sort((a,b)=>a.depth-b.depth);
      items[target]={id:target,depth:hatch[target],steps,minimumCertain:true,
        conditional:steps.some(step=>step.parents.some(parent=>parent.unknown))};
    }
  }

  function solve({pairs,speciesCount,stock}) {
    const result={items:[],unresolved:0};
    if (stock.length<2) return result;
    const groups=parentGroups(stock);
    const eligible = new Set(pairs.map(row=>row[0]).concat(pairs.map(row=>row[1])));
    const usable=stock.filter(p=>eligible.has(p.species));
    if (usable.length<2 || usable.every(p=>p.sex==='M') || usable.every(p=>p.sex==='F')) return result;
    const unassigned=[];
    const parents=[new Array(speciesCount),new Array(speciesCount)];
    for (const [index,group] of groups) {
      if (!eligible.has(index)) continue;
      parents[0][index]=group.M;
      parents[1][index]=group.F;
      if (group.singleUnknown) unassigned.push(group.singleUnknown);
    }
    function tryAssignment(items,assign) {
      unassigned.forEach((record,i)=>{
        const s=assign(record,i);
        parents[s][record.species]=record;
        parents[1-s][record.species]=null;
      });
      fixedAssignment(pairs,speciesCount,parents,items);
    }
    if (unassigned.length<=6) {
      const items=new Array(speciesCount);
      for (let mask=0;mask<2**unassigned.length;mask++) {
        tryAssignment(items,(_,i)=>(mask>>i)&1);
      }
      result.items=items.filter(Boolean).sort((a,b)=>a.depth-b.depth||a.id-b.id);
      return result;
    }
    const structural=WarehouseSolver.solve(pairs,speciesCount,groups.keys());
    let samples=null;
    for (let target=0;target<speciesCount;target++) {
      if (!Number.isFinite(structural.bredDepth[target])) continue;
      let steps=validateRoute(WarehouseSolver.route(structural,target,true),structural,groups);
      let minimumCertain=true;
      if (!steps) {
        // A handful of different concrete sex assignments can certify many
        // alternate routes at the species-only lower bound in one bulk pass.
        if (!samples) {
          samples=new Array(speciesCount);
          for (let bit=0;bit<3;bit++) for (let flip=0;flip<2;flip++) {
            tryAssignment(samples,(_,i)=>((i>>bit)&1)^flip);
          }
        }
        const candidate=samples[target];
        if (candidate?.depth===structural.bredDepth[target]) {
          result.items.push(candidate);
          continue;
        }
        // Another choice of parents or an extra generation may allow this
        // species even if the first species-level witness cannot be replayed.
        const exact=TraitSolver.solve({pairs,speciesCount,stock,target,traits:[],requireHatch:true});
        if (exact.status !== 'ok') {
          if (exact.status === 'unconfirmed-sex') {
            if (candidate) result.items.push({...candidate,minimumCertain:false});
            else result.unresolved++;
          }
          continue;
        }
        const founders=new Map(exact.founders.map(f=>[f.id,f]));
        steps=exact.steps.map(step=>({child:step.species,depth:step.depth,
          parents:step.parents.map(p=>({species:p.species,sex:p.sex,stockId:founders.get(p.id)?.stockId || null,
            unknown:founders.get(p.id)?.sex==='unknown'}))}));
        minimumCertain=!exact.minimumUnconfirmed;
        if (!minimumCertain && candidate && candidate.depth<steps[steps.length-1].depth) steps=candidate.steps;
      }
      const last=steps[steps.length-1];
      result.items.push({id:target,depth:last.depth,steps,minimumCertain,
        conditional:steps.some(s=>s.parents.some(p=>p.unknown))});
    }
    result.items.sort((a,b)=>a.depth-b.depth || a.id-b.id);
    return result;
  }
  return {solve};
})();

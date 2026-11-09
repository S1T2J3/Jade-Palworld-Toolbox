// The function is also serialized into the offline worker. All candidate records
// exist only in this calculation. A suggestion never modifies the warehouse.
function findBreedingGaps(request,reference,solve) {
  const {pairs,stock,target,speciesCount,traits=[],surgeryOptions=[]}=request;
  const wanted=new Set(traits), surgical=new Set(surgeryOptions.map(op=>op.key));
  const random=new Set(reference.random);
  const required=traits.filter(key=>!surgical.has(key));
  const stockTraits=new Set(stock.flatMap(row=>row.traits));
  const unknownSource=required.filter(key=>!stockTraits.has(key) && !random.has(key) &&
    !reference.fixed.some(keys=>keys.includes(key)));
  const recipes=pairs.filter(row=>row[2]===target);
  const limit=Number.isInteger(request.maxGenerations)?request.maxGenerations:Infinity;
  const options=[], attempts=[];
  if (!recipes.length || limit<1) return {options,unknownSource,tested:0};
  const needsTarget=recipes.every(([a,b])=>a===target || b===target);
  const speciesStock=Array.from({length:speciesCount},()=>[]);
  const distinct=new Set();
  for (const row of stock) {
    const signature=JSON.stringify([row.species,row.sex,row.traits.filter(key=>wanted.has(key)).sort()]);
    if (!distinct.has(signature) && speciesStock[row.species]) {
      speciesStock[row.species].push(row);distinct.add(signature);
    }
  }
  const mateSex=s=>s==='M'?'F':'M';
  const fitsSex=(row,sex)=>row.sex==='unknown'||row.sex==='any'||row.sex===sex;
  const targetParents=new Set(recipes.flatMap(row=>row.slice(0,2)));
  const fixed=reference.fixed;
  const donor=(species,sex)=>({species,sex,traits:[...fixed[species]]});
  function assignTraits(members,held,omit=[]) {
    // Include the species' compulsory passives in the four-slot capacity check,
    // even when some of those passives are irrelevant to the breeding goal.
    const inherited=new Set([...held,...members.flatMap(m=>m.traits)]);
    const unmet=required.filter(key=>!inherited.has(key) && !omit.includes(key));
    if (unmet.some(key=>!random.has(key))) return false;
    for (const key of unmet) {
      const member=members.find(m=>m.traits.length<4);
      if (!member) return false;
      member.traits.push(key);
    }
    return true;
  }
  const assumed=members=>members.reduce((n,m)=>n+m.traits.filter(key=>!fixed[m.species].includes(key)).length,0);
  const cost=o=>[o.members.length,assumed(o.members),o.members.filter(m=>!reference.sites[m.species]).length,
    o.assumptions?.length||0,o.generations??99];
  function order(a,b) {
    const x=cost(a), y=cost(b);
    for(let i=0;i<x.length;i++) if(x[i]!==y[i]) return x[i]-y[i];
    return 0;
  }
  function memberKey(m) {return m.species+'|'+m.traits.filter(k=>wanted.has(k)).sort().join(',');}
  const signature=o=>o.members.map(memberKey).sort().join('/');
  function record(option) {
    const key=signature(option);
    const index=options.findIndex(o=>signature(o)===key);
    if(index<0) options.push(option);
    else if(order(option,options[index])<0) options[index]=option;
    options.sort(order);
    if(options.length>3) options.pop();
  }
  function direct(a,sexA,parentA,b,sexB,parentB) {
    const members=[], parents=[];
    for(const [index,sex,existing] of [[a,sexA,parentA],[b,sexB,parentB]]) {
      if(!existing && index===target && !needsTarget) return;
      if(existing) parents.push({species:index,sex,id:existing.id});
      else {
        members.push(donor(index,sex));parents.push({species:index,sex});
      }
    }
    const held=[...(parentA?.traits||[]),...(parentB?.traits||[])];
    const assumptions=[];
    if(parentA?.sex==='unknown') assumptions.push({id:parentA.id,sex:sexA});
    if(parentB?.sex==='unknown') assumptions.push({id:parentB.id,sex:sexB});
    const present=new Set([...held,...members.flatMap(m=>m.traits)]);
    const optional=request.allowRandom?required.find(key=>!present.has(key)&&random.has(key)):null;
    for(const omit of optional?[[],[optional]]:[[]]) {
      const trial=members.map(m=>({...m,traits:[...m.traits]}));
      if(!assignTraits(trial,held,omit)) continue;
      const sources=new Set([...held,...trial.flatMap(m=>m.traits),...omit]);
      const after=surgeryOptions.filter(op=>wanted.has(op.key)&&!sources.has(op.key)).map(op=>op.key);
      const steps=[{depth:1,parents,species:target,traits:traits.filter(key=>sources.has(key)),randomTraits:omit}];
      record({members:trial,steps,generations:1,after,assumptions,randomHatch:Boolean(omit.length)});
    }
  }
  for(const [a,b,,requiredSex] of recipes) {
    for(const sex of requiredSex?[requiredSex]:['M','F']) {
      const other=mateSex(sex);
      for(const p of speciesStock[a]) if(fitsSex(p,sex)) direct(a,sex,p,b,other,null);
      for(const p of speciesStock[b]) if(fitsSex(p,other)) direct(a,sex,null,b,other,p);
      direct(a,sex,null,b,other,null);
    }
  }
  if(!solve || unknownSource.length || limit===1) return {options,unknownSource,tested:0};
  if(options.length===3 && options.every(o=>o.members.length===1 && !assumed(o.members) && !o.assumptions.length && o.generations===1))
    return {options,unknownSource,tested:0};

  // Prefer an acquisition that can form a direct parent of the target with one
  // existing animal. Fall back to other breeding pairs and guaranteed carriers.
  const priority=new Int16Array(speciesCount*2);
  const canMate=speciesStock.map(rows=>[rows.some(r=>fitsSex(r,'M')),rows.some(r=>fitsSex(r,'F'))]);
  const rate=(id,sex,score)=>{const i=id*2+(sex==='F'?1:0);priority[i]=Math.max(priority[i],score);};
  const isParent=new Set();
  for(const [a,b,c,sex] of pairs) {
    isParent.add(a);isParent.add(b);
    const score=c===target?3:targetParents.has(c)?2:1;
    if(sex!=='F') {
      if(canMate[a][0]) rate(b,'F',score);
      if(canMate[b][1]) rate(a,'M',score);
    }
    if(sex!=='M') {
      if(canMate[a][1]) rate(b,'M',score);
      if(canMate[b][0]) rate(a,'F',score);
    }
  }
  const needed=required.filter(k=>!stockTraits.has(k));
  const restricted=needed.filter(k=>!random.has(k));
  for(const id of isParent) {
    if(id===target && !needsTarget) continue;
    for(const sex of ['M','F']) {
      const member=donor(id,sex), score=priority[id*2+Number(sex==='F')];
      if(!score) continue;
      if(request.allowRandom && needed.some(k=>random.has(k))) {
        const onlyFixed=donor(id,sex);
        if(assignTraits([onlyFixed],stockTraits,needed.filter(k=>random.has(k)))) attempts.push({members:[onlyFixed],score});
      }
      if(assignTraits([member],stockTraits)) attempts.push({members:[member],score});
    }
  }
  attempts.sort((a,b)=>assumed(a.members)-assumed(b.members)||b.score-a.score||order(a,b));
  // Multiple non-random passives may belong to different fixed carriers. Build
  // a bounded frontier of separate donors instead of assuming all on one catch.
  if(restricted.length>1 && !attempts.length) {
    let groups=[[]];
    for(const key of restricted) {
      const carriers=[...isParent].filter(id=>fixed[id].includes(key) && (id!==target||needsTarget))
        .sort((a,b)=>Number(reference.sites[b])-Number(reference.sites[a]));
      const unique=new Map();
      for(const g of groups) {
        const expanded=g.some(id=>fixed[id].includes(key))?[g]:carriers.map(id=>[...g,id]);
        for(const ids of expanded) unique.set([...ids].sort((a,b)=>a-b).join(','),ids);
      }
      groups=[...unique.values()].sort((a,b)=>a.length-b.length).slice(0,12);
    }
    for(const g of groups) for(let mask=0;mask<(1<<g.length);mask++) {
      const members=g.map((id,i)=>donor(id,mask&(1<<i)?'F':'M'));
      if(assignTraits(members,stockTraits)) attempts.push({members});
    }
  }
  const start=Date.now(), used=new Set();
  let tested=0;
  for(const candidate of attempts) {
    if(tested>=32 || (tested && Date.now()-start>1800)) break;
    const addition=candidate.members;
    const key=addition.map(m=>memberKey(m)+'|'+m.sex).join('/');
    if(used.has(key)) continue;
    used.add(key);
    const temporary=new Map();
    const supplementary=addition.map((m,i)=>{
      let id='gap-proposal-'+i;
      while(stock.some(s=>s.id===id)) id+='-';
      temporary.set(id,m);
      return {...m,id};
    });
    const trial={...request,stock:[...stock,...supplementary],requireHatch:true,allowRandom:false};
    let answer=solve(trial);
    if(request.allowRandom && answer.status!=='ok') answer=solve({...trial,allowRandom:true});
    tested++;
    if(answer.status!=='ok' || answer.generations>limit) continue;
    const neededMembers=answer.founders.filter(f=>temporary.has(f.stockId)).map(f=>{
      const m=temporary.get(f.stockId);
      return {...m,traits:m.traits.filter(k=>fixed[m.species].includes(k)||f.requiredTraits.includes(k))};
    });
    if(!neededMembers.length) continue;
    const ids=new Map(answer.founders.map(f=>[f.id,f.stockId]));
    const steps=answer.steps.map(s=>({depth:s.depth,species:s.species,traits:s.traits,randomTraits:s.randomTraits,
      parents:s.parents.map(p=>({species:p.species,sex:p.sex,id:temporary.has(ids.get(p.id))?null:ids.get(p.id)}))}));
    const assumptions=answer.founders.filter(f=>f.assumedSex).map(f=>({id:f.stockId,sex:f.assumedSex}));
    record({members:neededMembers,steps,generations:answer.generations,after:(answer.surgery||[]).map(op=>op.key),
      randomHatch:!!answer.conditional,assumptions});
    if(options.length===3 && options.every(o=>o.members.length===1 && !assumed(o.members) && !o.assumptions.length)) break;
  }
  return {options,unknownSource,tested};
}

function createBreedingGapGuide({species,traitData,readDetails,readSpawnData,escape}) {
  const byTrait=new Map(traitData.traits.map(t=>[t.key,t]));
  const traitName=k=>byTrait.get(k)?.zh||k;
  const traitList=keys=>keys.map(k=>escape(traitName(k))).join('、');
  const sexLabel=sex=>sex==='M'?'雄性':'雌性';
  let reference,worker,url,serial=0;
  function release() {
    if(worker) {worker.terminate();worker=null;}
    if(url) {URL.revokeObjectURL(url);url=null;}
  }
  function cancel() {serial++;release();}
  function locationLink(index) {
    const key=encodeURIComponent(species[index].key);
    return reference.sites[index] ? '<a href="#spawns/'+key+'">查看刷新位置</a>' : '<a href="#paldex/'+key+'">查看图鉴</a>';
  }
  function show(root,request,result) {
    cancel();
    if(!root || !['missing-traits','unreachable-species','unreachable-traits','unconfirmed-sex'].includes(result.status)) return;
    if(!reference) {
      const profiles=readDetails().profiles, sites=readSpawnData().bySpecies;
      reference={random:traitData.traits.filter(t=>t.random).map(t=>t.key),
        fixed:species.map(p=>profiles[p.key]?.fixed||[]),sites:species.map(p=>Boolean(sites[p.key]?.length))};
    }
    const token=serial, panel=document.createElement('section');
    panel.className='bg-guide';panel.setAttribute('aria-label','补充亲代建议');root.append(panel);
    const who=key=>request.stock.find(p=>p.id===key);
    const absent=request.traits.filter(k=>!request.stock.some(s=>s.traits.includes(k))&&!request.surgeryOptions?.some(op=>op.key===k));
    const sources=absent.map(k=>{
      const indices=species.map((p,i)=>i).filter(i=>reference.fixed[i].includes(k));
      if(!indices.length) {
        const text=byTrait.get(k)?.random?'可随机出现，请核对个体实际携带的词条。':'未收录固定携带种，需确认已经取得的携带者。';
        return '<li><b>'+escape(traitName(k))+'</b> · '+text+'</li>';
      }
      const links=indices.map(i=>{
        const key=encodeURIComponent(species[i].key), available=reference.sites[i];
        return '<a href="#'+(available?'spawns/':'paldex/')+key+'">'+escape(species[i].zh)+(available?'（刷新位置）':'（图鉴）')+'</a>';
      });
      return '<li><b>'+escape(traitName(k))+'</b> · 固定携带：'+links.join('、')+'。</li>';
    }).join('');
    panel.innerHTML='<h4>补充亲代建议</h4>'+(sources?'<ul class="bg-sources">'+sources+'</ul>':'')+
      '<div class="bg-results" aria-live="polite">正在试算可补入的亲代…</div>';
    const body=panel.querySelector('.bg-results');
    function display(answer) {
      if(serial!==token || !panel.isConnected) return;
      if(!answer.options.length) {
        body.textContent=species[request.target].reason||'本次试算没有找到可验证的补入方案。可先核对仓库个体的性别、词条，再重新计算。';
        return;
      }
      const targetPairs=request.pairs.filter(row=>row[2]===request.target);
      const needsOwnSpecies=targetPairs.length && targetPairs.every(row=>row[0]===request.target||row[1]===request.target);
      const selfNote=needsOwnSpecies && !request.stock.some(p=>p.species===request.target)?
        '<p class="bg-foot">该物种的配方都需要本种作亲代。若取得的目标个体已具备所需词条，可直接登记入仓。</p>':'';
      body.innerHTML=selfNote+answer.options.map(option=>{
        const members=option.members.map(m=>{
          const base=reference.fixed[m.species], desired=m.traits.filter(key=>request.traits.includes(key));
          const fixed=desired.filter(k=>base.includes(k)), check=desired.filter(k=>!base.includes(k));
          const condition=[fixed.length?'图鉴固定携带 '+traitList(fixed):'',check.length?'须实际携带 '+traitList(check):''].filter(Boolean).join('，');
          return '<li><strong>'+escape(species[m.species].zh)+' · '+sexLabel(m.sex)+'</strong><span>'+ (condition||'词条不限')+'</span>'+locationLink(m.species)+'</li>';
        }).join('');
        const notes=[];
        for(const a of option.assumptions) {
          const p=who(a.id);
          if(p) notes.push('需确认仓库的'+escape(p.name||species[p.species].zh)+'为'+sexLabel(a.sex));
        }
        if(option.randomHatch) notes.push('途中需等待随机孵出指定词条');
        if(option.after.length) notes.push('最后手术添加 '+traitList(option.after));
        const steps=option.steps.map(step=>{
          const parents=step.parents.map(p=>escape(species[p.species].zh)+'（'+sexLabel(p.sex)+'）').join(' × ');
          return '<li>第 '+step.depth+' 代 · '+parents+' → '+escape(species[step.species].zh)+
            (step.traits.length?' · 保留 '+traitList(step.traits):'')+(step.randomTraits?.length?' · 随机孵出 '+traitList(step.randomTraits):'')+'</li>';
        }).join('');
        return '<article class="bg-option"><header><b>补入 '+option.members.length+' 只</b><span>可在 '+option.generations+' 代配出</span></header>'+
          '<ul class="bg-members">'+members+'</ul>'+(notes.length?'<p>'+notes.join('。')+'。</p>':'')+
          '<details><summary>查看这条试算路线</summary><ol>'+steps+'</ol></details></article>';
      }).join('')+'<p class="bg-foot">按以上性别与词条试算。取得个体后录入仓库，可重新计算完整路线。</p>';
    }
    const light=()=>display(findBreedingGaps(request,reference,null));
    try {
      const solverCode=document.getElementById('trait-solver-code').textContent;
      const source=solverCode+'\n;self.onmessage=e=>{try{self.postMessage({answer:('+
        findBreedingGaps.toString()+')(e.data.request,e.data.reference,TraitSolver.solve)});}catch{self.postMessage({error:true});}};';
      url=URL.createObjectURL(new Blob([source],{type:'text/javascript'}));worker=new Worker(url);
      worker.onmessage=({data})=>{if(token!==serial)return;release();if(data.error)light();else display(data.answer);};
      worker.onerror=e=>{e.preventDefault();if(token!==serial)return;release();light();};
      worker.postMessage({request,reference});
    } catch {release();light();}
  }
  return {show,cancel};
}

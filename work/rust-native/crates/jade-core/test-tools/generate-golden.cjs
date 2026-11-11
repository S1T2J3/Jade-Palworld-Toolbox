// Development-only oracle. The native product does not ship or execute Node/JS.
const fs=require('fs'),path=require('path'),vm=require('vm');
const ui=path.resolve(__dirname,'../../../../ui');
const traits=require(path.join(ui,'trait-solver.js'));
const warehouse=require(path.join(ui,'warehouse-solver.js'));
const crafting=require(path.join(ui,'item-crafting.js'));
const ctx=vm.createContext({});
for(const file of ['stat-calc.js','capture-calc.js','work-efficiency.js','relic.js'])vm.runInContext(fs.readFileSync(path.join(ui,file),'utf8'),ctx);
ctx.TraitSolver=traits;ctx.WarehouseSolver=warehouse;vm.runInContext(fs.readFileSync(path.join(ui,'offspring-solver.js'),'utf8')+'\n globalThis.nativeReferenceOffspring=OffspringSolver;',ctx);
let state=35215;function rnd(){state=(Math.imul(state,1664525)+1013904223)>>>0;return state/2**32}
const integer=n=>Math.floor(rnd()*n), rows=[];
for(let i=0;i<100;i++){
 const speciesCount=10,pairs=[];for(let a=0;a<speciesCount;a++)for(let b=a;b<speciesCount;b++)if(rnd()<.4)pairs.push([a,b,integer(speciesCount)]);
 const stock=Array.from({length:10},(_,j)=>({id:'u'+j,species:j,traits:[],sex:'unknown'}));
 const input={pairs,speciesCount,stock};rows.push({kind:'offspring',input,expected:ctx.nativeReferenceOffspring.solve(input)});
}
const base={pairs:[[1,2,0]],speciesCount:3,target:0,traits:[],stock:[{id:'owned',species:0,sex:'M',traits:[]},{id:'dad',species:1,sex:'M',traits:[]},{id:'mum',species:2,sex:'F',traits:[]}],preferHatch:true,allowRandom:false,randomEligible:['a','b']};
function plan(input){const inherited=traits.solve({...input,allowRandom:false}),owned=input.preferHatch&&inherited.status==='owned'?inherited:null,breeding=owned?{...input,requireHatch:true}:input,deterministic=owned?traits.solve({...breeding,allowRandom:false}):inherited;if(['ok','owned','invalid'].includes(deterministic.status))return deterministic;const result=input.allowRandom&&input.randomEligible.length?traits.solve(breeding):deterministic;return owned&&result.status!=='ok'?{...owned,hatchUnavailable:true}:result}
for(const input of [base,{...base,preferHatch:false},{...base,stock:[base.stock[0]]},{...base,traits:['a'],stock:[{...base.stock[0],traits:['a']},...base.stock.slice(1)],allowRandom:true},{...base,traits:['b'],surgeryOptions:[{key:'b',mode:'consume',cost:0}]}])rows.push({kind:'plan',input,expected:plan(input)});
for(let i=0;i<400;i++){
 const speciesCount=3+integer(6),pairs=[];for(let a=0;a<speciesCount;a++)for(let b=a;b<speciesCount;b++)if(rnd()<.7){const row=[a,b,integer(speciesCount)];if(rnd()<.12)row.push(rnd()<.5?'M':'F');pairs.push(row)}
 const keys=['a','b','c','d'],traitsWanted=keys.slice(0,integer(5));
 const stock=Array.from({length:1+integer(7)},(_,j)=>({id:'stock'+j,species:integer(speciesCount),traits:keys.filter(()=>rnd()<.35),sex:['M','F','unknown','any'][integer(4)]}));
 const input={pairs,speciesCount,stock,target:integer(speciesCount),traits:traitsWanted,allowRandom:rnd()<.5,randomEligible:keys.filter(()=>rnd()<.6),requireHatch:rnd()<.4,surgeryOptions:keys.filter(()=>rnd()<.2).map(key=>({key,mode:rnd()<.5?'consume':'coin',cost:integer(30)}))};
 rows.push({kind:'traits',input,expected:traits.solve(input)});
 rows.push({kind:'offspring',input:{pairs,speciesCount,stock},expected:ctx.nativeReferenceOffspring.solve({pairs,speciesCount,stock})});
 const owned=stock.map(r=>r.species),maxSteps=integer(5)+1;
 const plan=warehouse.solve(pairs,speciesCount,owned,maxSteps);
 rows.push({kind:'warehouse',input:{pairs,speciesCount,owned,maxSteps},expected:plan});
 rows.push({kind:'warehouseRoute',input:{plan,target:input.target,includeOwned:true},expected:warehouse.route(plan,input.target,true)});
}
for(let i=0;i<100;i++){
 const input={profile:{growth:[1+integer(200),1+integer(200),1+integer(200)],trust:[integer(5),integer(5),integer(5)]},config:{level:1+integer(80),stars:integer(5),trust:integer(11),awaken:rnd()<.5,iv:{hp:integer(101),attack:integer(101),defense:integer(101)},souls:{hp:integer(21),attack:integer(21),defense:integer(21),work:integer(21)}},passive:{hp:integer(50),attack:integer(100)-30,defense:integer(100)-30,work:integer(100)-50}};
 rows.push({kind:'stats',input,expected:ctx.calculatePalStats(input.profile,input.config,input.passive)});
 const capture={power:integer(100),level:1+integer(80),hp:integer(101),coefficient:rnd()*2,world:rnd()*3,status:integer(4)*.3,blocked:rnd()<.1};rows.push({kind:'capture',input:capture,expected:ctx.estimateCapture(capture)});
}
const byId=new Map([['a',{id:'a',recipes:[{qty:1,materials:[['b',2],['c',3]]}]}],['b',{id:'b',recipes:[{qty:4,materials:[['d',1]]}]}],['c',{id:'c',recipes:[{qty:2,materials:[['b',1],['d',3]]}]}],['d',{id:'d',recipes:[]}]]);
for(const quantity of [1,2,7,21])for(const choices of [new Map(),new Map([['b',-1]])]){const p=crafting.planItemCraft(byId,'a',quantity,choices);rows.push({kind:'craft',input:{items:Object.fromEntries(byId),target:'a',quantity,choices:Object.fromEntries(choices)},expected:{steps:p.steps,external:[...p.external],cycles:p.cycles,nodes:[...p.nodes.values()].map(v=>({...v,recipe:v.recipe||null,blocked:[...v.blocked].sort()}))}})}
const tier={slots:[{pool:'a',probability:100},{pool:'a',probability:20},{pool:'b',probability:50}]},pools={a:[['wood',2,5,2],['stone',1,3,1]],b:[['wood',2,4,1]]};
rows.push({kind:'relic',input:{tier,pools},expected:[...ctx.relicProductOdds(tier,pools).values()]});
const draws=[.1,.2,.3,.1,.9,.8,.3,.6,.4];let pos=0;rows.push({kind:'relicSim',input:{tier,pools,draws},expected:[...ctx.simulateRelic(tier,pools,()=>draws[pos++]) ]});
const work={levels:{'生火':[50,100,200],'搬运':[1,2,5],'牧场':[1,1.5,2,2.5]},modifiers:{a:{work:50,ranch:1,movement:20},b:{work:-150}}};
for(const job of Object.keys(work.levels))for(const level of [0,1,2])for(const keys of [[],['a'],['a','a','b']])rows.push({kind:'work',input:{data:work,job,level,keys},expected:ctx.calculateWorkEfficiency(work,job,level,keys)});
fs.mkdirSync(path.join(__dirname,'../tests'),{recursive:true});fs.writeFileSync(path.join(__dirname,'../tests/golden.json'),JSON.stringify(rows));console.log(rows.length);

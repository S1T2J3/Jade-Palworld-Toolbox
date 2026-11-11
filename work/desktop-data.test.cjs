const assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
const source=fs.readFileSync(require.resolve('./ui/desktop-data.js'),'utf8');
function environment({cores=8,fail=null}={}){
 let time=0,uid=0;const tasks=new Map(),listeners=new Map(),starts=[],tries=new Map();let decodes=0;
 const timer=(fn,delay=0)=>{const id=++uid;tasks.set(id,{fn,at:time+delay});return id};
 const blocks=['code-camera','code-map','map-data','map-images','code-items','item-catalog-data','code-updates'].map(id=>({id,dataset:id.startsWith('code-')?{localModule:id}:{localTable:id},textContent:'',hasAttribute(key){return key==='data-local-table'?!id.startsWith('code-'):false}}));
 const status={hidden:true,querySelector:s=>s==='span'?{textContent:''}:{hidden:true,addEventListener(){}}};
 const document={hidden:false,readyState:'complete',querySelectorAll:()=>blocks,getElementById:id=>id==='atlas-load-status'?status:blocks.find(b=>b.id===id),addEventListener(type,fn){if(!listeners.has(type))listeners.set(type,[]);listeners.get(type).push(fn)},createElement:()=>({remove(){}}),head:{append(script){const id=script.src;starts.push({id,time});tries.set(id,(tries.get(id)||0)+1);timer(()=>{if(fail===id && tries.get(id)===1){script.onerror();return;}const b=blocks.find(b=>b.id===id);if(b?.dataset.localTable)b.textContent=id==='map-images'?'{"palpagos":"assets/map.webp"}':'{}';script.onload();},20)}}};
 const window={requestIdleCallback:fn=>timer(()=>fn({timeRemaining:()=>50}),1),cancelIdleCallback:id=>tasks.delete(id)};
 class Image{set src(value){timer(()=>this.onload(),20)}async decode(){decodes++}}
 const context={document,window,navigator:{hardwareConcurrency:cores},performance:{now:()=>time},location:{hash:'#lookup'},setTimeout:timer,clearTimeout:id=>tasks.delete(id),requestAnimationFrame:fn=>timer(fn,16),Image,Event};
 vm.runInNewContext(source,context);const loader=context.createAtlasDataLoader();
 const flush=async()=>{for(let i=0;i<12;i++)await Promise.resolve()};
 async function advance(ms){const end=time+ms;await flush();let steps=0;while(true){const ready=[...tasks].filter(([id,t])=>t.at<=end).sort((a,b)=>a[1].at-b[1].at)[0];if(!ready)break;if(++steps>2000)throw Error('timer loop');const [id,t]=ready;time=t.at;tasks.delete(id);t.fn();await flush();}time=end;await flush()}
 const event=type=>(listeners.get(type)||[]).forEach(fn=>fn(new Event(type)));
 return {loader,starts,tries,status,document,event,advance,get time(){return time},get decodes(){return decodes}};
}
(async()=>{
 const a=environment();a.loader.startBackground();a.loader.startBackground();await a.advance(1200);assert.equal(a.starts.length,0,'no optional reads before initial frame and grace period');await a.advance(80);assert.equal(a.starts[0].id,'code-camera');assert.equal(a.starts.length,1);
 a.event('keydown');const active=a.starts.length;await a.advance(600);assert.equal(a.starts.length,active,'input yields before the next batch');await a.advance(900);assert(a.starts.length>active);
 a.document.hidden=true;a.event('visibilitychange');const hidden=a.starts.length;await a.advance(3000);assert.equal(a.starts.length,hidden,'hidden windows do not launch more work');a.document.hidden=false;a.event('visibilitychange');await a.advance(4000);assert(a.starts.some(s=>s.id==='item-catalog-data'));assert(!a.starts.some(s=>s.id==='code-updates'),'installer module is never a preload target');assert.equal(a.decodes,1,'only default map decoded');assert(a.starts.every(s=>a.tries.get(s.id)===1),'resources coalesce');
 const low=environment({cores:2});low.loader.startBackground();await low.advance(10000);assert.equal(low.decodes,0,'low-core profiles do not force a full map bitmap decode');
 const fail=environment({fail:'map-data'});fail.loader.startBackground();await fail.advance(7000);assert.equal(fail.tries.get('map-data'),1);assert.equal(fail.status.hidden,true,'background failure does not open retry UI');let complete=false;assert.equal(fail.loader.wait('map',()=>complete=true),true);await fail.advance(100);assert.equal(fail.tries.get('map-data'),2);assert(complete,'foreground retry succeeds after background failure');
 const concurrent=environment();const first=concurrent.loader.load('map-data'),second=concurrent.loader.load('map-data');assert.equal(first,second,'in-flight reads reused');await concurrent.advance(30);await first;assert.equal(concurrent.tries.get('map-data'),1);
 console.log('Idle preload: first-frame grace, serial batches, input/visibility yielding, low-core image budget, installer exclusion, shared reads and foreground retry passed.');
})().catch(e=>{console.error(e);process.exitCode=1});

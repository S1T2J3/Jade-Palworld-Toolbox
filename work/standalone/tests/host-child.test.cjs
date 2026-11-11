const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path'),vm=require('node:vm');
const source=fs.readFileSync(path.resolve(__dirname,'../shell/chrome/content/PalAtlasHostChild.sys.mjs'),'utf8');
const Actor=vm.runInNewContext(source.replace('export class ','class ')+'\nPalAtlasHostChild;',{JSWindowActorChild:class {}});
const id='pal-'+'a'.repeat(64),unknown='pal-'+'b'.repeat(64);
function make(open=false,saves=[{id}]){
 const queries=[],reports=[],dialog={open,dataset:{gameScanRequest:'scan-1'}},win={navigator:{userActivation:{isActive:true}}};win.top=win;
 const doc={nodePrincipal:{URI:{scheme:'file'}},getElementById:()=>dialog,createEvent:()=>({initCustomEvent(type,bubbles,cancelable,detail){this.type=type;this.detail=detail;}}),dispatchEvent:event=>reports.push(event)};
 const actor=new Actor();actor.document=doc;actor.contentWindow=win;actor.sendAsyncMessage=()=>queries.push({name:'async'});actor.sendQuery=async(name,data)=>{queries.push({name,data});return name==='PalAtlas:ReadGameSaves'?{state:'ok',saves,defaultDirectoryExists:true}:{state:'ok',rows:[]};};
 const send=(type,detail)=>actor.handleEvent({type,target:doc,detail:JSON.stringify(detail)});
 return {actor,queries,reports,dialog,win,send,doc};
}
const tick=()=>new Promise(resolve=>setImmediate(resolve));
(async()=>{
 const a=make();a.send('pal-read-game-saves',{mode:'folder',requestId:'scan-1'});assert.equal(a.queries.length,0);
 a.win.navigator.userActivation.isActive=false;a.send('pal-read-game-saves',{mode:'auto',requestId:'scan-1'});assert.equal(a.queries.length,0);
 a.win.navigator.userActivation.isActive=true;a.send('pal-read-game-saves',{mode:'auto',requestId:'scan-1'});await tick();assert.equal(a.queries.length,1);assert.equal(a.reports.length,1);
 a.win.navigator.userActivation.isActive=false;a.send('pal-read-terminal',{gameId:id,requestId:'terminal-1'});await tick();assert.equal(a.queries.length,2);
 a.send('pal-read-terminal',{gameId:unknown,requestId:'terminal-2'});await tick();assert.equal(a.queries.length,2);
 a.win.navigator.userActivation.isActive=true;for(const type of ['pal-new-window','pal-open-legacy','pal-save-backup'])a.send(type,{});await tick();assert.equal(a.queries.length,2);
 const empty=make(false,[]);empty.send('pal-read-game-saves',{mode:'auto',requestId:'scan-1'});await tick();assert.equal(empty.queries.length,1);assert.equal(empty.reports.length,1);assert(!JSON.parse(empty.reports[0].detail).pickingDefault);
 const foreground=make(true,[]);foreground.send('pal-read-game-saves',{mode:'auto',requestId:'scan-1'});await tick();assert.equal(foreground.queries.length,2);assert.equal(foreground.queries[1].data.mode,'default-folder');
 console.log('Host bridge keeps activation/dialog restrictions and detected-world whitelist; closed-dialog auto refresh never opens a folder picker.');
})().catch(e=>{console.error(e);process.exitCode=1;});

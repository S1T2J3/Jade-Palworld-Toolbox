// The content process supplies only a scan mode. The native file picker and
// this profile's last successful choice are the only sources of custom paths.
import { Subprocess } from "resource://gre/modules/Subprocess.sys.mjs";

const rememberedFolder="palatlas.gameSaves.lastFolder";
async function resolveCompressedNames(window,result) {
  const metadata=result.pending||[];
  delete result.pending;
  if(!metadata.length) return;
  let worker;
  try {
    for(const {compressed,rawLength,...world} of metadata) {
      if(!worker) worker=new window.Worker("chrome://palatlas/content/game-meta-worker.mjs",{type:"module"});
      const name=await new Promise(resolve=>{
        const finish=value=>{window.clearTimeout(timer);resolve(value);};
        const timer=window.setTimeout(()=>{worker.terminate();worker=null;finish(null);},3000);
        worker.onmessage=event=>finish(event.data);
        worker.onerror=event=>{event.preventDefault();worker.terminate();worker=null;finish(null);};
        worker.postMessage({compressed,rawLength});
      });
      if(name) result.saves.push({...world,name});
      else result.warnings.push("未能读取此存档中的世界名："+world.path);
    }
  } finally {worker?.terminate();}
}
async function scan(window,directory) {
  const proc=await Subprocess.call({
    command:Services.env.get("PAL_ATLAS_LAUNCHER"),
    arguments:directory?["--scan-game-folder",directory]:["--scan-game-saves"]
  });
  let timedOut=false;
  const timer=window.setTimeout(()=>{timedOut=true;proc.kill(0).catch(()=>{});},45000);
  try {
    await proc.stdin.close();
    const chunks=[];
    for(;;) {
      const next=await proc.stdout.readString();
      if(!next) break;
      chunks.push(next);
    }
    const {exitCode}=await proc.wait();
    if(timedOut) throw new Error("读取存档超时，请检查存档磁盘或共享目录后重试。");
    if(exitCode!==0) throw new Error("存档读取程序未能完成。请稍后重试或手动选择存档目录。");
    const result=JSON.parse(chunks.join(""));
    if(!Array.isArray(result.saves) || !Array.isArray(result.roots) || !Array.isArray(result.warnings))
      throw new Error("存档读取程序返回了无效数据。");
    await resolveCompressedNames(window,result);
    return result;
  } finally {window.clearTimeout(timer);}
}
async function chooseFolder(browser) {
  const picker=Cc["@mozilla.org/filepicker;1"].createInstance(Ci.nsIFilePicker);
  picker.init(browser.browsingContext,"选择幻兽帕鲁存档目录",Ci.nsIFilePicker.modeGetFolder);
  const result=await new Promise(resolve=>picker.open(resolve));
  return result===Ci.nsIFilePicker.returnOK?picker.file?.path:null;
}
export async function readGameSaves(window,browser,mode) {
  try {
    if(mode==="folder") {
      const directory=await chooseFolder(browser);
      if(!directory) return {state:"cancelled"};
      const result=await scan(window,directory);
      if(result.saves.length) Services.prefs.setStringPref(rememberedFolder,directory);
      return {state:"ok",...result};
    }
    const result=await scan(window,null);
    const directory=Services.prefs.getStringPref(rememberedFolder,"");
    const normalize=p=>p.replace(/[\\/]+$/,"").toLowerCase();
    if(directory && !result.roots.some(path=>normalize(path)===normalize(directory))) {
      try {
        const custom=await scan(window,directory);
        const saves=new Map(result.saves.map(row=>[row.id,row]));
        for(const row of custom.saves) if(!saves.has(row.id)) saves.set(row.id,row);
        result.saves=[...saves.values()];
        result.warnings=[...new Set([...result.warnings,...custom.warnings])];
        result.roots=[...new Set([...result.roots,...custom.roots])];
      }catch(error){result.warnings.push("上次选择的目录未能读取。"+error.message);}
    }
    return {state:"ok",...result};
  } catch(error) {
    return {state:"error",message:error.message||"读取游戏存档失败。"};
  }
}

// The content process supplies only a scan mode. The native file picker and
// this profile's last successful choice are the only sources of custom paths.
import { readDesktopResult } from "chrome://palatlas/content/read-result.sys.mjs";

const rememberedFolder="palatlas.gameSaves.lastFolder";
const pickerLocations=new WeakMap();
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
  const args=directory?["--scan-game-folder",directory]:["--scan-game-saves"];
  const result=await readDesktopResult(args,32*1024*1024,45000);
  if(!Array.isArray(result.saves) || !Array.isArray(result.roots) || !Array.isArray(result.warnings))
    throw new Error("存档读取程序返回了无效数据。");
  await resolveCompressedNames(window,result);
  return result;
}
async function chooseFolder(browser,mode) {
  const picker=Cc["@mozilla.org/filepicker;1"].createInstance(Ci.nsIFilePicker);
  picker.init(browser.browsingContext,"选择幻兽帕鲁存档目录",Ci.nsIFilePicker.modeGetFolder);
  const location=pickerLocations.get(browser);
  const local=Services.env.get("LOCALAPPDATA");
  const defaultDirectory=location?.defaultDirectory||(local?PathUtils.join(local,"Pal","Saved","SaveGames"):null);
  const candidates=new Set(mode==="default-folder"?[]:[
    location?.selectedDirectory,Services.prefs.getStringPref(rememberedFolder,"")
  ]);
  // On a new installation even the Pal folder may not exist. Stop at LocalAppData.
  let candidate=defaultDirectory;
  for(let depth=0;candidate&&depth<4;depth++) {
    candidates.add(candidate);
    candidate=PathUtils.parent(candidate);
  }
  for(const path of candidates) {
    if(!path) continue;
    try {
      if((await IOUtils.stat(path)).type!=="directory") continue;
      const folder=Cc["@mozilla.org/file/local;1"].createInstance(Ci.nsIFile);
      folder.initWithPath(path);
      picker.displayDirectory=folder;
      break;
    } catch { /* A saved location may have been removed or disconnected. */ }
  }
  const result=await new Promise(resolve=>picker.open(resolve));
  return result===Ci.nsIFilePicker.returnOK?picker.file?.path:null;
}
export async function readGameSaves(window,browser,mode) {
  try {
    if(mode==="folder" || mode==="default-folder") {
      const directory=await chooseFolder(browser,mode);
      if(!directory) return {state:"cancelled"};
      const result=await scan(window,directory);
      pickerLocations.set(browser,{defaultDirectory:result.defaultDirectory,selectedDirectory:directory});
      if(result.saves.length) Services.prefs.setStringPref(rememberedFolder,directory);
      return {state:"ok",...result,selectedDirectory:directory};
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
    pickerLocations.set(browser,{
      defaultDirectory:result.defaultDirectory,
      selectedDirectory:result.saves.length?null:result.defaultDirectory
    });
    return {state:"ok",...result};
  } catch(error) {
    return {state:"error",message:error.message||"读取游戏存档失败。"};
  }
}

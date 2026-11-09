import { Subprocess } from "resource://gre/modules/Subprocess.sys.mjs";
import { setTimeout, clearTimeout } from "resource://gre/modules/Timer.sys.mjs";
import { readDesktopResult } from "chrome://palatlas/content/read-result.sys.mjs";

// Level files never cross IPC. Only the native reader opens them and only
// the compact terminal records are sent back to the handbook.
async function runJSON(command,args,input,timeout) {
  const proc=await Subprocess.call({command,arguments:args});
  let expired=false;
  const timer=setTimeout(()=>{expired=true;proc.kill(0).catch(()=>{});},timeout);
  try {
    if(input) await proc.stdin.write(JSON.stringify(input));
    await proc.stdin.close();
    const chunks=[];
    let length=0;
    for(;;) {
      const next=await proc.stdout.readString();
      if(!next) break;
      length+=next.length;
      if(length>64*1024*1024) throw Error("终端数据量超出本次读取范围。");
      chunks.push(next);
    }
    const {exitCode}=await proc.wait();
    if(expired) throw Error("读取帕鲁终端超时，请待游戏保存完成后重试。");
    let result;
    try {result=JSON.parse(chunks.join(""));}catch{throw Error("终端读取程序未能完成。请检查存档是否完整，再重新检测。");}
    if(typeof result?.error==="string" && result.error) throw Error(result.error);
    if(exitCode!==0) throw Error("此终端存档未能完整解析，请重新检测。");
    return result;
  } finally {clearTimeout(timer);proc.kill(0).catch(()=>{});}
}

async function sources(world) {
  const args=["--terminal-sources",world.platform==="Xbox / Game Pass"?"wgs":"folder",world.id,world.path];
  const result=await readDesktopResult(args,32*1024*1024,30000);
  if(result?.error) throw Error(result.error);
  return result;
}

let lastRead=null;
async function readWorld(world) {
  const manifest=await sources(world);
  if(!manifest.snapshot || !Array.isArray(manifest.level) || !Array.isArray(manifest.players)) throw Error("世界存档清单无效，请重新检测。");
  // A refresh still checks every file stamp, including newly added player files.
  if(lastRead?.snapshot===manifest.snapshot && lastRead.game===world.id) return lastRead.result;
  const shell=Services.env.get("PAL_ATLAS_SHELL");
  const reader=PathUtils.join(shell,"tools","pal-terminal-reader.exe");
  const parsed=await runJSON(reader,[],{level:manifest.level,players:manifest.players},240000);
  if(!Array.isArray(parsed.records) || !Array.isArray(parsed.owners)) throw Error("终端读取程序返回了不完整的数据。");
  if((await sources(world)).snapshot!==manifest.snapshot)
    throw Error("游戏刚刚写入了新存档。请重新检测以读取同一次保存的数据。");
  const owners=parsed.owners.map(owner=>({id:owner.id,name:owner.name,isLocal:owner.id===manifest.localOwner}));
  const result={state:"ok",rows:parsed.records,owners,warnings:parsed.warnings||[],revision:Date.now()};
  lastRead={game:world.id,snapshot:manifest.snapshot,result};
  return result;
}

// Concurrent windows share a read of the same world. Different large worlds
// run in turn to keep peak memory comparable to one decoder process.
const pending=new Map();
let previous=Promise.resolve();
export function readTerminal(world) {
  const key=world.id+"\0"+world.path;
  if(!pending.has(key)) {
    const task=previous.then(()=>readWorld(world))
      .catch(error=>({state:"error",message:error.message||"无法读取帕鲁终端。"}))
      .finally(()=>pending.delete(key));
    pending.set(key,task);
    previous=task.then(()=>{});
  }
  return pending.get(key);
}

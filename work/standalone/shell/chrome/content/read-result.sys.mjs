import { Subprocess } from "resource://gre/modules/Subprocess.sys.mjs";
import { setTimeout, clearTimeout } from "resource://gre/modules/Timer.sys.mjs";

// A Windows GUI child can finish successfully without a usable stdout handle.
// Both ends derive this one-time result path. It is never supplied by page content.
export async function readDesktopResult(args,limit,timeout) {
  const root=Services.env.get("PAL_ATLAS_DATA")||PathUtils.profileDir;
  const id=Services.uuid.generateUUID().toString().replace(/[-{}]/g,"");
  const response=PathUtils.join(root,"ipc","read-"+id+".json");
  const proc=await Subprocess.call({
    command:Services.env.get("PAL_ATLAS_LAUNCHER"),arguments:[...args,root,id]
  });
  let expired=false;
  const timer=setTimeout(()=>{expired=true;proc.kill(0).catch(()=>{});},timeout);
  try {
    await proc.stdin.close();
    const {exitCode}=await proc.wait();
    if(expired) throw new Error("读取游戏存档超时，请检查存档磁盘后重试。");
    if(exitCode!==0) throw new Error("存档读取程序未能完成，请重新检测。");
    if((await IOUtils.stat(response)).size>limit) throw new Error("存档读取结果超过本次读取范围。");
    return await IOUtils.readJSON(response);
  } catch(error) {
    await proc.kill(0).catch(()=>{});
    throw error;
  } finally {
    clearTimeout(timer);
    await IOUtils.remove(response,{ignoreAbsent:true}).catch(()=>{});
  }
}

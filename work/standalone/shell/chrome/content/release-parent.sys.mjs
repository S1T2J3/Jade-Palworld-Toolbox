import { Subprocess } from "resource://gre/modules/Subprocess.sys.mjs";
import { setTimeout } from "resource://gre/modules/Timer.sys.mjs";

let startupCheck=null,checked=false,restartInProgress=false;

async function nativeRelease(command) {
  const root=Services.env.get("PAL_ATLAS_DATA");
  const id=Services.uuid.generateUUID().toString().replace(/[-{}]/g,"");
  const response=PathUtils.join(root,"updates","ipc",id+".json");
  const proc=await Subprocess.call({command:Services.env.get("PAL_ATLAS_LAUNCHER"),
    arguments:["--release",command,root,id]});
  try {
    await proc.stdin.close();
    const {exitCode}=await proc.wait();
    if(exitCode!==0) throw new Error("Update service failed");
    if((await IOUtils.stat(response)).size>1024*1024) throw new Error("Unexpected update response");
    return await IOUtils.readJSON(response);
  } catch(error) {
    try {await proc.kill(0);}catch{}
    throw error;
  } finally {await IOUtils.remove(response,{ignoreAbsent:true}).catch(()=>{});}
}

export async function runReleaseAction({action,enabled,version}) {
  if(action==="auto") {
    if(startupCheck)return startupCheck;
    if(checked)return {...await nativeRelease("status"),action:"status"};
    // This privileged module is shared by all windows in the running application.
    const task=(async()=>{
      let result=await nativeRelease("auto");
      // A manual check or another control window may briefly hold the updater mutex.
      for(const pause of [1000,4000,10000]){
        if(result.state!=="busy" || !result.auto)break;
        await new Promise(resolve=>setTimeout(resolve,pause));
        if(startupCheck!==task)return {...await nativeRelease("status"),action:"status"};
        result=await nativeRelease("auto");
      }
      if(startupCheck===task)checked=result.state!=="busy";
      return result;
    })().finally(()=>{
      if(startupCheck===task)startupCheck=null;
    });
    startupCheck=task;
    return task;
  }
  if(action==="restart") {
    if(typeof version!=="string" || !/^\d{1,5}(?:\.\d{1,5}){2,3}$/.test(version)) throw new Error("Invalid update version");
    if(restartInProgress)return {state:"busy",message:"正在准备重启，请稍候。"};
    restartInProgress=true;
    try {
      const result=await nativeRelease("verify-restart:"+version);
      if(result.state!=="restarting")return result;
      Services.startup.quit(Ci.nsIAppStartup.eAttemptQuit,78);
      return Services.startup.attemptingQuit?result:{state:"error",message:"窗口仍在使用中，请稍后再试。"};
    } finally {restartInProgress=false;}
  }
  const commands={status:"status",check:"check",download:"download",
    setAccess:"setAccess",useMachine:"useMachine",clearAccess:"clearAccess"};
  const command=action==="setAuto" && typeof enabled==="boolean" ? (enabled?"auto-on":"auto-off") :
    Object.hasOwn(commands,action) ? commands[action] : null;
  if(!command) throw new Error("Unknown update action");
  const result=await nativeRelease(command);
  if(command==="auto-on" && result.auto===true && result.state!=="busy") {
    checked=false;startupCheck=null;
  }
  return result;
}

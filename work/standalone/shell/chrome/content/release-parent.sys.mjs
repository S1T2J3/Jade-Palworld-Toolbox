import { Subprocess } from "resource://gre/modules/Subprocess.sys.mjs";

export async function runReleaseAction({action,enabled}) {
  const commands={status:"status",auto:"auto",check:"check",download:"download",
    setAccess:"setAccess",useMachine:"useMachine",clearAccess:"clearAccess"};
  const command=action==="setAuto" && typeof enabled==="boolean" ? (enabled?"auto-on":"auto-off") :
    Object.hasOwn(commands,action) ? commands[action] : null;
  if (!command) throw new Error("Unknown update action");
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

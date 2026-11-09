export class PalAtlasHostChild extends JSWindowActorChild {
  handleEvent(event) {
    const doc=this.document, win=this.contentWindow;
    if (!doc || doc.nodePrincipal.URI?.scheme!=="file" || win!==win.top) return;
    if ((event.type==="pal-theme-changed" || event.type==="DOMContentLoaded") && event.target===doc) {
      const theme=doc.documentElement.dataset.theme||"moon";
      if(typeof theme==="string" && theme.length<=10)this.sendAsyncMessage("PalAtlas:Theme",{theme});
      if (event.type==="DOMContentLoaded") win.setTimeout(()=>{
        if (this.document===doc) this.release(doc,{action:"auto"});
      },20000);
      return;
    }
    if (event.type==="keydown" && event.isTrusted) {
      if (event.altKey && !event.ctrlKey && !event.shiftKey && event.key==="ArrowRight") {
        event.preventDefault();win.history.forward();
      }
      if (event.ctrlKey && !event.altKey) {
        if(!event.shiftKey && event.key.toLowerCase()==="n") {
          event.preventDefault();this.sendAsyncMessage("PalAtlas:NewWindow");return;
        }
        const command=({"0":"reset","=":"up","+":"up","-":"down"})[event.key];
        if(command){event.preventDefault();this.sendAsyncMessage("PalAtlas:Zoom",{command});}
      }
      return;
    }
    if (event.type==="click" && event.isTrusted) {
      const link=event.target.closest?.("a[href]");
      if (link && /^https?:\/\//i.test(link.href)) {
        event.preventDefault();this.sendAsyncMessage("PalAtlas:OpenSource",{url:link.href});
      }
      return;
    }
    if (event.type==="pal-release-request" && event.target===doc) {
      let request;
      try {request=JSON.parse(event.detail);} catch {return;}
      if (typeof request?.requestId!=="string" || request.requestId.length>100 ||
          !["status","check","download","setAuto","setAccess","useMachine","clearAccess"].includes(request.action) ||
          (request.action!=="status" && !win.navigator.userActivation.isActive) ||
          (request.action==="setAuto" && typeof request.enabled!=="boolean")) return;
      this.release(doc,request);
      return;
    }
    if (event.target!==doc || !doc.getElementById("world-dialog")?.open) return;
    const report=(type,result)=>{
      const e=doc.createEvent("CustomEvent");
      e.initCustomEvent(type,false,false,JSON.stringify(result));doc.dispatchEvent(e);
    };
    if(event.type==="pal-read-terminal") {
      let request;
      try {request=JSON.parse(event.detail);}catch{return;}
      if(typeof request?.requestId!=="string" || !request.requestId || request.requestId.length>100 ||
        typeof request.gameId!=="string" || !/^pal-[a-f0-9]{64}$/.test(request.gameId)) return;
      const query={gameId:request.gameId};
      this.sendQuery("PalAtlas:ReadTerminal",query)
        .then(result=>report("pal-terminal-result",{...result,...query,requestId:request.requestId}))
        .catch(()=>report("pal-terminal-result",{...query,requestId:request.requestId,state:"error",message:"终端读取未完成，请重新检测存档。"}));
      return;
    }
    if(!win.navigator.userActivation.isActive) return;
    if(event.type==="pal-read-game-saves") {
      let request;
      try {request=JSON.parse(event.detail);}catch{return;}
      if(!["auto","folder"].includes(request?.mode) ||
         !(Number.isSafeInteger(request.requestId) || (typeof request.requestId==="string" && request.requestId.length<=100))) return;
      this.sendQuery("PalAtlas:ReadGameSaves",{mode:request.mode})
        .then(result=>report("pal-game-saves-result",{...result,requestId:request.requestId}))
        .catch(()=>report("pal-game-saves-result",{state:"error",message:"未能读取游戏存档。请重试。",requestId:request.requestId}));
      return;
    }
    if (event.type==="pal-new-window") this.sendAsyncMessage("PalAtlas:NewWindow");
    if (event.type==="pal-open-legacy") {
      this.sendQuery("PalAtlas:Legacy").catch(()=>report("pal-backup-result",{state:"error",text:"未能打开旧版迁移窗口。请在原浏览器中打开原手册并导出全部世界。"}));
    }
    if (event.type==="pal-save-backup") {
      let payload;
      try {payload=JSON.parse(event.detail);} catch {return;}
      if (payload?.backup?.format!=="pal-atlas-worlds" || !Array.isArray(payload.backup.worlds)) return;
      event.preventDefault();
      this.sendQuery("PalAtlas:Save",{name:payload.name,text:JSON.stringify(payload.backup,null,2)})
        .then(result=>report("pal-backup-result",result))
        .catch(()=>report("pal-backup-result",{state:"error",text:"备份未能保存。请重新打开世界档案后重试。"}));
    }
  }
  async release(doc,{action,enabled,requestId}) {
    let result;
    try {result=await this.sendQuery("PalAtlas:Release",{action,enabled});}
    catch {result={state:"error",message:"更新服务暂时不可用。"};}
    if (this.document!==doc) return;
    const event=doc.createEvent("CustomEvent");
    event.initCustomEvent("pal-release-result",false,false,JSON.stringify({...result,requestId}));
    doc.dispatchEvent(event);
  }
}

import { saveDesktopBackup } from "chrome://palatlas/content/backup-parent.sys.mjs";
import { readGameSaves } from "chrome://palatlas/content/game-saves-parent.sys.mjs";
import { readTerminal } from "chrome://palatlas/content/terminal-parent.sys.mjs";
function fileAt(path) {
  const file=Cc["@mozilla.org/file/local;1"].createInstance(Ci.nsIFile);
  file.initWithPath(path);return file;
}
export class PalAtlasHostParent extends JSWindowActorParent {
  async receiveMessage({name,data}) {
    const context=this.browsingContext,browser=context.embedderElement,doc=browser?.ownerDocument,win=doc?.defaultView;
    const file=fileAt(Services.env.get("PAL_ATLAS_PAGE"));file.normalize();
    const permitted=Services.io.newFileURI(file), manager=this.manager;
    const exactFile=uri=>{
      if(!uri?.schemeIs("file") || !uri.equalsExceptRef(permitted))return false;
      const local=uri.QueryInterface(Ci.nsIFileURL).file;local.normalize();
      return local.equals(file);
    };
    if (context!==context.top || doc?.documentURI!=="chrome://palatlas/content/main.xhtml" ||
        !doc.nodePrincipal.isSystemPrincipal || doc.getElementById("atlas")!==browser ||
        context.currentWindowGlobal!==manager || !manager.documentPrincipal.isContentPrincipal ||
        !exactFile(manager.documentURI) || !exactFile(manager.documentPrincipal.URI)) {
      throw new Error("Unexpected application document");
    }
    if (name==="PalAtlas:Theme") {
      win.setTitlebarTheme(data?.theme);
      return;
    }
    if (name==="PalAtlas:Release") {
      const {runReleaseAction}=ChromeUtils.importESModule("chrome://palatlas/content/release-parent.sys.mjs");
      return runReleaseAction(data);
    }
    if (name==="PalAtlas:Zoom") {
      const now=browser.fullZoom||1;
      browser.fullZoom=data.command==="reset"?1:Math.min(3,Math.max(0.5,Math.round((now+(data.command==="up"?0.1:-0.1))*10)/10));return;
    }
    if(name==="PalAtlas:ReadGameSaves") {
      if(!["auto","folder","default-folder"].includes(data?.mode)) throw new Error("Unknown scan mode");
      let result;
      if(data.mode==="auto") {
        if(!this.gameScan) this.gameScan=readGameSaves(win,browser,"auto").finally(()=>{this.gameScan=null;});
        result=await this.gameScan;
        this.offerDefaultFolder=result.state==="ok" && result.defaultDirectoryExists && result.saves.length===0;
      } else {
        if(this.gamePicker || (data.mode==="default-folder" && !this.offerDefaultFolder)) return {state:"cancelled"};
        this.offerDefaultFolder=false;
        this.gamePicker=true;
        try{result=await readGameSaves(win,browser,data.mode);}finally{this.gamePicker=false;}
      }
      if(result.state==="ok") this.detectedWorlds=new Map(result.saves.map(world=>[world.id,world]));
      return result;
    }
    if(name==="PalAtlas:ReadTerminal") {
      const world=this.detectedWorlds?.get(data?.gameId);
      if(!world) return {state:"error",message:"请先检测存档，再读取所选世界的帕鲁终端。"};
      return readTerminal(world);
    }
    if (name==="PalAtlas:NewWindow") {
      Services.ww.openWindow(null,"chrome://palatlas/content/main.xhtml","_blank","chrome,dialog=no,resizable,all",null);
      return;
    }
    if (name==="PalAtlas:Save") {
      if (this.saving) return {state:"cancelled",text:"另一个保存窗口已打开。"};
      if (typeof data?.name!=="string" || typeof data?.text!=="string") throw new Error("Invalid backup");
      const backup=JSON.parse(data.text);
      if (backup?.format!=="pal-atlas-worlds" || backup.version!==1 || !Array.isArray(backup.worlds) ||
          !backup.worlds.some(w=>w.id===backup.activeWorldId)) throw new Error("Invalid backup");
      this.saving=true;
      try {return await saveDesktopBackup(win,browser,data.name,data.text);} finally {this.saving=false;}
    }
    let args;
    if (name==="PalAtlas:Legacy") args=["--open-legacy"];
    else if (name==="PalAtlas:OpenSource" && typeof data?.url==="string" && /^https?:\/\//i.test(data.url)) {
      const uri=Services.io.newURI(data.url);
      if (!uri.host) throw new Error("Invalid external source");
      args=["--open-link",uri.asciiSpec];
    } else throw new Error("Unknown desktop action");
    const process=Cc["@mozilla.org/process/util;1"].createInstance(Ci.nsIProcess);
    process.init(fileAt(Services.env.get("PAL_ATLAS_LAUNCHER")));
    process.runwAsync(args,args.length);
    return null;
  }
}

import assert from "node:assert/strict";
import {readFile} from "node:fs/promises";

globalThis.JSWindowActorChild=class {};
globalThis.JSWindowActorParent=class {};
const source=new URL("../shell/chrome/content/",import.meta.url);
const {PalAtlasHostChild}=await import(new URL("PalAtlasHostChild.sys.mjs",source));
const child=new PalAtlasHostChild(), commands=[];
const win={navigator:{userActivation:{isActive:false}}};
win.top=win;
const doc={nodePrincipal:{URI:{scheme:"file"}},getElementById(){return null;}};
Object.assign(child,{document:doc,contentWindow:win,release(_doc,request){commands.push(request.action);}});
function request(action,extra={}) {
  child.handleEvent({type:"pal-release-request",target:doc,detail:JSON.stringify({requestId:"test",action,...extra})});
}
request("status");request("download");request("setAccess");request("restart",{version:"1.0.6"});
assert.deepEqual(commands,["status"]);
win.navigator.userActivation.isActive=true;
request("setAuto",{enabled:1});request("arbitrary");request("auto");request("download");request("setAuto",{enabled:false});
assert.deepEqual(commands,["status","download","setAuto"]);
request("restart",{version:"not-a-version"});request("restart",{version:"1.0.6"});
assert.deepEqual(commands,["status","download","setAuto","restart"]);
win.top={};request("status");win.top=win;
doc.nodePrincipal.URI.scheme="https";request("download");
assert.equal(commands.length,4);

// Evaluate the Gecko parent actor with platform dependencies stubbed. No native
// process should be reached until every origin, principal and window check passes.
const actorSource=(await readFile(new URL("PalAtlasHostParent.sys.mjs",source),"utf8")).replace(/^import .*;\r?$/gm,"");
const {PalAtlasHostParent}=await import("data:text/javascript,"+encodeURIComponent(actorSource));
const file={normalize(){},initWithPath(){},equals(other){return other===file;}};
const uri={schemeIs(s){return s==="file";},equalsExceptRef(other){return other===uri;},QueryInterface(){return {file};}};
globalThis.Services={env:{get(){return "C:\\Fixture\\app\\index.html";}},io:{newFileURI(){return uri;}}};
globalThis.Cc={"@mozilla.org/file/local;1":{createInstance(){return file;}}};
globalThis.Ci={};let reached=0;
globalThis.ChromeUtils={importESModule(){return {runReleaseAction(){reached++;return "ok";}};}};
const manager={documentURI:uri,documentPrincipal:{isContentPrincipal:true,URI:uri}};
const owner={documentURI:"chrome://palatlas/content/main.xhtml",nodePrincipal:{isSystemPrincipal:true},getElementById(){return browser;}};
const browser={ownerDocument:owner},context={currentWindowGlobal:manager,embedderElement:browser};
context.top=context;
const parent=Object.assign(new PalAtlasHostParent(),{browsingContext:context,manager});
const call=()=>parent.receiveMessage({name:"PalAtlas:Release",data:{action:"status"}});
assert.equal(await call(),"ok");
for(const [target,key,invalid] of [[context,"top",{}],[context,"currentWindowGlobal",{}],[owner,"documentURI","file:///other.html"],
  [owner.nodePrincipal,"isSystemPrincipal",false],[manager.documentPrincipal,"isContentPrincipal",false],
  [manager,"documentURI",{...uri,equalsExceptRef(){return false;}}],[manager.documentPrincipal,"URI",{...uri,equalsExceptRef(){return false;}}]]) {
  const original=target[key];target[key]=invalid;
  await assert.rejects(call,/Unexpected application document/);
  target[key]=original;
}
assert.equal(reached,1);
console.log("Passed privileged origin, owning window and transient user activation checks.");

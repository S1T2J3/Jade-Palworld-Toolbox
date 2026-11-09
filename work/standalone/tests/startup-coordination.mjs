import assert from "node:assert/strict";
import {readFile} from "node:fs/promises";
import {runInNewContext} from "node:vm";

const file=new URL("../shell/chrome/content/release-parent.sys.mjs",import.meta.url);
const code=(await readFile(file,"utf8")).replace(/^import .*;\r?$/gm,"")
  .replace(/async function nativeRelease\(command\)[\s\S]*?\n\}\r?\n/,"")
  .replace("export async function runReleaseAction","async function runReleaseAction");
const trace=[],pauses=[];
let responses=["busy","busy","current"],auto=true;
const action=runInNewContext(code+"\nrunReleaseAction;",{
  async nativeRelease(command){
    trace.push(command);
    if(command==="auto")return {state:responses.shift()||"current",auto};
    if(command==="auto-on")auto=true;
    if(command==="auto-off")auto=false;
    return {state:"current",auto};
  },
  setTimeout(fn,ms){pauses.push(ms);queueMicrotask(fn);}
});
await Promise.all([action({action:"auto"}),action({action:"auto"})]);
assert.deepEqual(trace,["auto","auto","auto"]);
assert.deepEqual(pauses,[1000,4000]);
assert.equal((await action({action:"auto"})).action,"status");
await action({action:"setAuto",enabled:false});await action({action:"setAuto",enabled:true});
responses=["busy","busy","busy","busy"];
assert.equal((await action({action:"auto"})).state,"busy");
assert.deepEqual(pauses,[1000,4000,1000,4000,10000]);
responses=["current"];assert.equal((await action({action:"auto"})).state,"current");assert.equal(trace.at(-1),"auto");
console.log("Passed shared startup check, limited busy retries, cache reuse and re-enabling auto after startup.");

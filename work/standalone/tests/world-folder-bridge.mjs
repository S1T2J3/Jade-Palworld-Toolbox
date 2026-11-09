import assert from "node:assert/strict";

globalThis.JSWindowActorChild=class {};
const {PalAtlasHostChild}=await import("../shell/chrome/content/PalAtlasHostChild.sys.mjs");
const emptyScan={state:"ok",defaultDirectory:"C:\\Fixture\\Pal\\Saved\\SaveGames",defaultDirectoryExists:true,saves:[]};

async function exercise(scan,options={}) {
  const dialog={open:true,dataset:{gameScanRequest:"first"}};
  const doc={getElementById(){return dialog;}};
  const actor=new PalAtlasHostChild();
  actor.document=doc;
  const modes=[],responses=[];
  actor.sendQuery=async(name,{mode})=>{
    assert.equal(name,"PalAtlas:ReadGameSaves");
    modes.push(mode);
    if (mode==="auto") {
      if (options.replaceDialog) dialog.dataset.gameScanRequest="second";
      return scan;
    }
    return {state:"cancelled"};
  };
  await actor.readWorlds(doc,{mode:options.mode||"auto",requestId:"first"},result=>{
    responses.push(result);
    if (options.closeOnResult) dialog.open=false;
  });
  return {modes,responses};
}

const fallback=await exercise(emptyScan);
assert.deepEqual(fallback.modes,["auto","default-folder"]);
assert.equal(fallback.responses[0].defaultDirectory,emptyScan.defaultDirectory);
assert.equal(fallback.responses[0].pickingDefault,true);
assert.equal(fallback.responses[1].state,"cancelled");
for (const scan of [{...emptyScan,defaultDirectoryExists:false},{...emptyScan,saves:[{id:"world"}]},{state:"error"}]) {
  const result=await exercise(scan);
  assert.deepEqual(result.modes,["auto"]);
  assert.deepEqual(result.responses,[scan]);
}
assert.deepEqual((await exercise(emptyScan,{mode:"folder"})).modes,["folder"]);
assert.deepEqual(await exercise(emptyScan,{replaceDialog:true}),{modes:["auto"],responses:[]});
assert.deepEqual((await exercise(emptyScan,{closeOnResult:true})).modes,["auto"]);
console.log("Empty, missing, successful, cancelled and stale world scans passed.");

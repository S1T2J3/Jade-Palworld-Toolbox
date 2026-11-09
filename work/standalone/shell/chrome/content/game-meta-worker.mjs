import { decodeWorldName } from "./decoder/save-meta.mjs";

// The worker receives only metadata bytes. It has no need for a file path.
self.onmessage=async ({data})=>{
  try {
    const name=await decodeWorldName(data.compressed,data.rawLength);
    self.postMessage(typeof name==="string" && name.trim()?name:null);
  } catch {self.postMessage(null);}
};

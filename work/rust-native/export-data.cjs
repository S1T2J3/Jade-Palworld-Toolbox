// Build-time conversion only. The native application never runs JavaScript.
const fs=require('fs'),path=require('path'),vm=require('vm'),crypto=require('crypto');
const repo=path.resolve(__dirname,'../..');
const sourceArg=process.argv[2];
const candidates=fs.existsSync(path.join(repo,'build'))?fs.readdirSync(path.join(repo,'build')).filter(n=>n.startsWith('content-')&&fs.existsSync(path.join(repo,'build',n,'index.html'))).map(n=>path.join(repo,'build',n)).sort((a,b)=>fs.statSync(b).mtimeMs-fs.statSync(a).mtimeMs):[];
const source=sourceArg?path.resolve(sourceArg):candidates[0];
if(!source)throw Error('First build the existing offline data, then pass its content directory: node work/rust-native/export-data.cjs <directory-containing-index.html>');
const dest=path.join(__dirname,'data');fs.mkdirSync(dest,{recursive:true});
const html=fs.readFileSync(path.join(source,'index.html'),'utf8');
const checks={tables:{},assets:{}};
for(const m of html.matchAll(/<script([^>]*)>([\s\S]*?)<\/script>/g)){
 const id=m[1].match(/id="([^"]+)"/);if(!id||!m[1].includes('application/json'))continue;
 const src=m[1].match(/data-local-table="([^"]+)"/);let raw=m[2];
 if(src){const document={getElementById:()=>({set textContent(t){raw=t}})};vm.runInNewContext(fs.readFileSync(path.join(source,src[1]),'utf8'),{document});}
 const value=JSON.parse(raw);
 fs.writeFileSync(path.join(dest,id[1]+'.json'),raw);checks.tables[id[1]]={bytes:Buffer.byteLength(raw),sha256:crypto.createHash('sha256').update(raw).digest('hex')};
}
fs.mkdirSync(path.join(dest,'assets'),{recursive:true});
for(const name of fs.readdirSync(path.join(source,'assets'))){
 const bytes=fs.readFileSync(path.join(source,'assets',name));fs.writeFileSync(path.join(dest,'assets',name),bytes);
 checks.assets[name]=crypto.createHash('sha256').update(bytes).digest('hex');
}
const db=JSON.parse(fs.readFileSync(path.join(dest,'db.json'),'utf8'));
const {createRecipeIndex}=require('../ui/recipe-index.js');
fs.writeFileSync(path.join(dest,'pairs.json'),JSON.stringify(createRecipeIndex(db).all()));
const notices=JSON.parse(fs.readFileSync(path.join(repo,fs.existsSync(path.join(repo,'rust-announcements.json'))?'rust-announcements.json':'announcements.json'),'utf8'));
fs.writeFileSync(path.join(dest,'announcements.json'),JSON.stringify(notices));
const ranks=JSON.parse(fs.readFileSync(path.join(__dirname,'../ui/trait-ranks.js'),'utf8').match(/const passiveRankTextures = (\[[\s\S]*?\]);/)[1]);
const rankPaths=ranks.map((s,i)=>{if(!s)return '';const name=`passive-rank-${i}.webp`;fs.writeFileSync(path.join(dest,'assets',name),Buffer.from(s.split(',')[1],'base64'));return 'assets/'+name;});
fs.writeFileSync(path.join(dest,'trait-ranks.json'),JSON.stringify(rankPaths));
fs.writeFileSync(path.join(dest,'manifest.json'),JSON.stringify(checks,null,2));
console.log(JSON.stringify({tables:Object.keys(checks.tables).length,assets:Object.keys(checks.assets).length,pals:db.species.length,pairs:createRecipeIndex(db).all().length}));

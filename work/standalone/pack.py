"""Pack the application shell and content. Mozilla supplies the runtime separately."""
from hashlib import sha256
from pathlib import Path
import re, sys, zipfile

base=Path(__file__).resolve().parent
out=base/"build";out.mkdir(exist_ok=True)

def files(root):
    return {p.relative_to(root).as_posix():p for p in root.rglob("*") if p.is_file()}

def pack(name,entries,cache=False):
    target=out/(name+".zip")
    # Cached engine is invalidated whenever a runtime/host source changes.
    fingerprint=sha256(("\n".join(k+":"+str(v.stat().st_size)+":"+str(v.stat().st_mtime_ns) for k,v in sorted(entries.items()))).encode()).hexdigest()
    mark=out/(name+".cache")
    if cache and mark.exists() and target.exists() and mark.read_text()==fingerprint:
        return sha256(target.read_bytes()).hexdigest()
    stage=out/(name+".partial")
    with zipfile.ZipFile(stage,"w",allowZip64=True) as dest:
        for key,source in sorted(entries.items()):
            info=zipfile.ZipInfo(key,(2026,11,8,0,0,0))
            info.compress_type=zipfile.ZIP_STORED if source.suffix.lower() in (".webp",".ico",".jpg",".png") else zipfile.ZIP_DEFLATED
            dest.writestr(info,source.read_bytes(),compress_type=info.compress_type,compresslevel=6)
    stage.replace(target);mark.write_text(fingerprint)
    return sha256(target.read_bytes()).hexdigest()

runtime_files=files(base/"runtime-config")
runtime_files["notices/firefox-runtime.sha256"]=base/"firefox-runtime.sha256"
shell=base/"shell"
runtime_files["palatlas-shell/chrome.manifest"]=shell/"chrome.manifest"
for name,p in files(shell/"chrome").items():
    runtime_files["palatlas-shell/chrome/"+name]=p
runtime_files["palatlas-shell/chrome/content/backup-parent.sys.mjs"]=base.parent/"migration/backup-parent.sys.mjs"
decoder=base.parent/"game-saves/decoder"
for name in ("save-meta.mjs","ooz-bytes.mjs"):
    runtime_files["palatlas-shell/chrome/content/decoder/"+name]=decoder/name
for name in ("NOTICE.txt","LICENSE-ooz.txt","ooz-wasm-source.zip"):
    runtime_files["notices/ooz/"+name]=decoder/name
terminal=base.parent/"terminal-import/parser"
runtime_files["palatlas-shell/tools/pal-terminal-reader.exe"]=terminal/"pal-terminal-reader.exe"
for name in ("NOTICE.txt","terminal-reader-source.zip"):
    runtime_files["notices/pal-terminal-reader/"+name]=terminal/name
runtime_files["notices/MPL-2.0.txt"]=base/"upstream/MPL-2.0.txt"
for name,source in {"PalCalc.txt":"data/LICENSE.txt","Palworld-Atlas-Data.txt":"data/atlas-license.txt",
                    "Pal-Atlas.txt":"map-data/LICENSE","PalSphere.md":"item-data/sphere-license.md",
                    "pako.txt":"vendor/pako/LICENSE"}.items():
    runtime_files["notices/"+name]=base.parent/source
runtime_files["chrome/icons/default/palatlas-main.ico"]=base.parent/"desktop/atlas.ico"
content=files(Path(sys.argv[1]).resolve())
assert "index.html" in content and "firefox.exe" not in runtime_files
for key in content:
    if key!="index.html" and not re.fullmatch(r"(assets|tables|modules)/[^/]*[0-9a-f]{24,64}\.(webp|ico|png|jpg|js)",key): raise ValueError("Non-versioned dependency: "+key)
digests=[pack("engine",runtime_files,True),pack("content",content)]
(out/"hashes").write_text("\n".join(digests)+"\n",encoding="ascii")
print("Engine",len(runtime_files),"files",sum(p.stat().st_size for p in runtime_files.values()),"bytes ->",(out/"engine.zip").stat().st_size)
print("Content",len(content),"files",sum(p.stat().st_size for p in content.values()),"bytes ->",(out/"content.zip").stat().st_size)

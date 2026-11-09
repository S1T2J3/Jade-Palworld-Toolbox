using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Net;
using System.Reflection;
using System.Security.Cryptography;
using System.Text;
using System.Text.RegularExpressions;
using System.Threading;
using System.Web.Script.Serialization;

internal sealed class UpdateService {
    internal const string FullName="Jade-Palworld-Toolbox.exe", FastName="Jade-Palworld-Toolbox-Fast.exe";
    const int MetaLimit=1024*1024, NoticeLimit=256*1024;
    internal delegate void Fetch(string path,string accept,long limit,Stream output,bool asset);
    // Field names mirror the JSON contracts read by JavaScriptSerializer.
#pragma warning disable 0649
    internal class Notice {
        public string id,title,body,publishedAt,url;
        // Preserve JSON types so only the literal true opts a notice into a popup.
        public object popup;
    }
    internal class Notices { public Notice[] announcements; }
    internal class Asset {public string name,digest,state;public long id,size;}
    internal class Release {public string tag_name,body; public bool? prerelease,draft; public Asset[] assets;}
#pragma warning restore 0649
    internal class Ready { public string version,folder,fastHash; }
    internal class Snapshot {
        public string state="idle",currentVersion,latestVersion,readyVersion,checkedAt,message,releaseUrl,releaseNotes,accessMode;
        public string downloadKind;
        public long downloadBytes;
        public bool auto;
        public Notice[] announcements;
    }
    readonly JavaScriptSerializer json=new JavaScriptSerializer{MaxJsonLength=MetaLimit,RecursionLimit=24};
    readonly string directory,repository,baselineOverride;
    readonly Version current;
    readonly ReleaseAccess access;
    readonly string lockName;
    internal UpdateService(string root,string repo,Version current,string baselineOverride=null) {
        if(!Regex.IsMatch(repo,@"\A[a-zA-Z0-9][a-zA-Z0-9-]{0,38}/[a-zA-Z0-9][a-zA-Z0-9_.-]{0,99}\z")) throw new InvalidDataException();
        repository=repo;
        this.current=current;
        this.baselineOverride=baselineOverride;
        directory=Path.Combine(root,"updates",Key(repo.ToLowerInvariant()));
        access=new ReleaseAccess(directory,repo);
        lockName=@"Local\JadeRelease_"+Key(directory.ToUpperInvariant());
    }
    internal static UpdateService For(string root) {
        Assembly assembly=Assembly.GetExecutingAssembly();
        using(var input=new StreamReader(assembly.GetManifestResourceStream("pal.update.repository"),Encoding.UTF8))
            return new UpdateService(root,input.ReadToEnd().Trim(),assembly.GetName().Version);
    }
    static string Hex(byte[] bytes) {return BitConverter.ToString(bytes).Replace("-","").ToLowerInvariant();}
    static string Key(string value) {
        using(var hash=SHA256.Create()) return Hex(hash.ComputeHash(Encoding.UTF8.GetBytes(value))).Substring(0,24);
    }
    internal static void AtomicWrite(string path,byte[] bytes) {
        Directory.CreateDirectory(Path.GetDirectoryName(path));
        string temp=path+"."+Guid.NewGuid().ToString("N")+".tmp";
        try {
            File.WriteAllBytes(temp,bytes);
            if(File.Exists(path)) File.Replace(temp,path,null);
            else File.Move(temp,path);
        } finally {if(File.Exists(temp)) File.Delete(temp);}
    }
    T Read<T>(string name) where T:class {
        try {
            using(var file=new FileStream(Path.Combine(directory,name),FileMode.Open,FileAccess.Read,FileShare.ReadWrite|FileShare.Delete)) {
                if(file.Length>MetaLimit) return null;
                using(var reader=new StreamReader(file,Encoding.UTF8)) return json.Deserialize<T>(reader.ReadToEnd());
            }
        } catch {return null;}
    }
    void Save(string name,object value) {AtomicWrite(Path.Combine(directory,name),Encoding.UTF8.GetBytes(json.Serialize(value)));}
    static string Display(Version v) {return v.Revision>0?v.ToString(4):v.ToString(3);}
    internal static Version Parse(string value) {
        Version version;
        if(value==null || !Regex.IsMatch(value,@"\A(0|[1-9]\d{0,4})(\.(0|[1-9]\d{0,4})){2,3}\z",RegexOptions.CultureInvariant)
            || !Version.TryParse(value,out version)) return null;
        return new Version(version.Major,version.Minor,version.Build,Math.Max(0,version.Revision));
    }
    bool IsNew(string v) {Version target=Parse(v);return target!=null && target>current;}
    static bool ValidReady(Ready r) {
        return r!=null && Parse(r.version)!=null && r.folder!=null && Regex.IsMatch(r.folder,@"\Av-[a-f0-9]{32}\z")
            && r.fastHash!=null && Regex.IsMatch(r.fastHash,@"\A[a-f0-9]{64}\z");
    }
    Snapshot Cached() {
        var result=Read<Snapshot>("cache.json")??new Snapshot();
        result.currentVersion=Display(current);
        result.auto=!File.Exists(Path.Combine(directory,"auto-off"));
        result.accessMode=access.Mode;
        Ready ready=Read<Ready>("ready.json");
        result.readyVersion=ValidReady(ready) && IsNew(ready.version) && !File.Exists(Path.Combine(directory,ready.folder,"rejected")) &&
            File.Exists(Path.Combine(directory,ready.folder,FullName)) && File.Exists(Path.Combine(directory,ready.folder,FastName))?ready.version:null;
        if(result.readyVersion!=null && (!IsNew(result.latestVersion) || Parse(result.readyVersion)>=Parse(result.latestVersion))) result.state="ready";
        else if(result.state=="ready" || result.state=="available" || result.state=="current")
            result.state=IsNew(result.latestVersion)?"available":result.latestVersion!=null?"current":"idle";
        return result;
    }
    static string ReadRemote(Fetch get,string path,string accept,int limit) {
        using(var stream=new MemoryStream()) {
            get(path,accept,limit,stream,false);
            return new UTF8Encoding(false,true).GetString(stream.ToArray()).TrimStart('\uFEFF');
        }
    }
    static string Clip(string value,int maximum) {
        if(value==null)return null;
        return value.Length>maximum?value.Substring(0,maximum):value;
    }
    string SafeUrl(string value) {
        Uri uri;
        if(value==null || value.Length>1000 || !Uri.TryCreate(value,UriKind.Absolute,out uri) ||
            uri.Scheme!="https" || !uri.IsDefaultPort || uri.Host!="github.com" || uri.UserInfo.Length!=0 ||
            uri.Query.Length!=0 || !uri.AbsolutePath.StartsWith("/"+repository+"/",StringComparison.OrdinalIgnoreCase)) return null;
        return uri.AbsoluteUri;
    }
    Notice[] ParseNotices(string source) {
        Notices parsed=json.Deserialize<Notices>(source);
        if(parsed==null || parsed.announcements==null || parsed.announcements.Length>30) throw new InvalidDataException();
        var ids=new HashSet<string>(StringComparer.Ordinal);
        foreach(Notice n in parsed.announcements) {
            DateTimeOffset date;
            if(n==null || n.id==null || !Regex.IsMatch(n.id,@"\A[\w.-]{1,80}\z") || !ids.Add(n.id) ||
                String.IsNullOrWhiteSpace(n.title) || n.title.Length>160 || String.IsNullOrWhiteSpace(n.body) || n.body.Length>10000 ||
                n.publishedAt==null || n.publishedAt.Length>40 ||
                !DateTimeOffset.TryParse(n.publishedAt,CultureInfo.InvariantCulture,DateTimeStyles.AssumeUniversal,out date)) throw new InvalidDataException();
            n.publishedAt=date.UtcDateTime.ToString("o");n.url=SafeUrl(n.url);
            n.popup=Object.Equals(n.popup,true);
        }
        return parsed.announcements.OrderByDescending(n=>n.publishedAt,StringComparer.Ordinal).ToArray();
    }
    static bool ValidAsset(Asset asset,long maximum) {
        return asset!=null && asset.id>0 && asset.state=="uploaded" && asset.size>0 && asset.size<=maximum &&
            asset.digest!=null && Regex.IsMatch(asset.digest,@"\Asha256:[a-fA-F0-9]{64}\z");
    }
    static Asset Select(Release release,string name,long maximum) {
        Asset asset=(release.assets??new Asset[0]).SingleOrDefault(a=>a!=null && a.name==name);
        if(!ValidAsset(asset,maximum))throw new InvalidDataException();
        return asset;
    }
    static string FileHash(string path) {
        using(var file=File.OpenRead(path)) using(var sha=SHA256.Create()) return Hex(sha.ComputeHash(file));
    }
    static void Transfer(Fetch get,Asset asset,string path) {
        using(var file=new FileStream(path,FileMode.CreateNew,FileAccess.Write,FileShare.None)) {
            get("releases/assets/"+asset.id.ToString(CultureInfo.InvariantCulture),"application/octet-stream",asset.size,file,true);
            if(file.Length!=asset.size) throw new EndOfStreamException();
        }
    }
    static bool Matches(string folder,Asset asset,Version version) {
        string file=Path.Combine(folder,asset.name);
        if(new FileInfo(file).Length!=asset.size || !String.Equals(FileHash(file),asset.digest.Substring(7),StringComparison.OrdinalIgnoreCase))return false;
        try {return AssemblyName.GetAssemblyName(file).Version==version;}
        catch(BadImageFormatException){return false;}
    }
    IEnumerable<string> BaseFiles() {
        if(baselineOverride!=null){yield return baselineOverride;yield break;}
        Assembly assembly=Assembly.GetExecutingAssembly();
        using(var metadata=assembly.GetManifestResourceStream("pal.companion")) {
            if(metadata==null){yield return assembly.Location;yield break;}
            using(var reader=new StreamReader(metadata)) {
                reader.ReadLine();
                string name=reader.ReadLine(),dir=Path.GetDirectoryName(assembly.Location);
                foreach(string candidate in new[]{name,FullName,"Jade 幻兽帕鲁工具箱.exe","帕鲁配种手册.exe"}.Distinct(StringComparer.OrdinalIgnoreCase))
                    if(!String.IsNullOrEmpty(candidate) && candidate==Path.GetFileName(candidate)) yield return Path.Combine(dir,candidate);
            }
        }
    }
    bool TryDelta(Fetch get,Release release,string path,Version version,Asset full,Asset fast,ref long downloaded) {
        long maximum=full.size+fast.size-1;
        var options=release.assets.Where(a=>ValidAsset(a,maximum) && a.name!=null &&
            Regex.IsMatch(a.name,@"\AJade-Delta-from-[a-f0-9]{64}\.jdp\z")).ToLookup(a=>a.name,StringComparer.Ordinal);
        if(options.Count==0)return false;
        foreach(string file in BaseFiles()) {
            if(!File.Exists(file))continue;
            FileStream basis;
            try {basis=File.OpenRead(file);}
            catch(IOException){continue;}catch(UnauthorizedAccessException){continue;}
            using(basis)using(var hash=SHA256.Create()) {
                if(basis.Length==0 || basis.Length>512L*1024*1024)continue;
                string sourceHash=Hex(hash.ComputeHash(basis));
                var matches=options[ReleaseDelta.Prefix+sourceHash+".jdp"];
                if(matches.Count()!=1)continue;
                Asset asset=matches.First();
                string patch=Path.Combine(path,"download.jdp");
                // Transport and permission failures escape: retry the small transfer later.
                Transfer(get,asset,patch);
                downloaded+=asset.size;
                try {
                    if(!String.Equals(FileHash(patch),asset.digest.Substring(7),StringComparison.OrdinalIgnoreCase))return false;
                    try {ReleaseDelta.Apply(basis,patch,path,full.size,fast.size);}
                    catch(InvalidDataException){return false;}catch(EndOfStreamException){return false;}
                    return Matches(path,full,version) && Matches(path,fast,version);
                } finally {File.Delete(patch);}
            }
        }
        return false;
    }
    void Stage(Fetch get,Release release,Version target,Snapshot result) {
        Asset full=Select(release,FullName,512L*1024*1024),fast=Select(release,FastName,8L*1024*1024);
        string folder="v-"+Guid.NewGuid().ToString("N"),path=Path.Combine(directory,folder);
        Directory.CreateDirectory(path);
        bool committed=false;
        try {
            long downloaded=0;
            bool delta=TryDelta(get,release,path,target,full,fast,ref downloaded);
            if(!delta) foreach(Asset asset in new[]{fast,full}) {
                string file=Path.Combine(path,asset.name);
                File.Delete(file); // A rejected patch may have reconstructed part of this file.
                Transfer(get,asset,file);
                if(!Matches(path,asset,target))throw new InvalidDataException();
                downloaded+=asset.size;
            }
            var previous=Read<Ready>("ready.json");
            if(ValidReady(previous) && !File.Exists(Path.Combine(directory,previous.folder,"rejected"))) Save("previous.json",previous);
            AtomicWrite(Path.Combine(Path.GetDirectoryName(directory),"has-prepared"),new byte[0]);
            Save("ready.json",new Ready{version=Display(target),folder=folder,fastHash=fast.digest.Substring(7).ToLowerInvariant()});
            committed=true;
            result.downloadKind=delta?"delta":"full";
            result.downloadBytes=downloaded;
            TrimPackages(folder);
        } finally {if(!committed && Directory.Exists(path)) Directory.Delete(path,true);}
    }
    void TrimPackages(string active) {
        var previous=Read<Ready>("previous.json");
        string running=Path.GetDirectoryName(Assembly.GetExecutingAssembly().Location);
        try {
            foreach(string dir in Directory.GetDirectories(directory,"v-*")) {
                string name=Path.GetFileName(dir);
                if(!Regex.IsMatch(name,@"\Av-[a-f0-9]{32}\z") || name==active || (ValidReady(previous) && name==previous.folder) ||
                    String.Equals(dir,running,StringComparison.OrdinalIgnoreCase) || (File.GetAttributes(dir)&FileAttributes.ReparsePoint)!=0) continue;
                try {
                    foreach(string file in new[]{FullName,FastName,"rejected"})File.Delete(Path.Combine(dir,file));
                    Directory.Delete(dir,false);
                } catch(IOException){} catch(UnauthorizedAccessException){}
            }
        } catch(IOException){} catch(UnauthorizedAccessException){}
    }
    static bool Denied(Exception e) {
        var web=e as WebException;
        var response=web==null?null:web.Response as HttpWebResponse;
        if(response==null)return false;
        using(response) {return response.StatusCode==HttpStatusCode.Unauthorized ||
            response.StatusCode==HttpStatusCode.NotFound || (response.StatusCode==HttpStatusCode.Forbidden && response.Headers["X-RateLimit-Remaining"]!="0");}
    }
    internal void Refresh(Snapshot result,Fetch get,bool prepare) {
        string noticeMessage=null;
        try {result.announcements=ParseNotices(ReadRemote(get,"contents/announcements.json","application/vnd.github.raw+json",NoticeLimit));}
        catch(Exception e) {
            Denied(e);
            noticeMessage="本次未能刷新公告，已保留本地公告。";
        }
        Release release=json.Deserialize<Release>(ReadRemote(get,"releases/latest","application/vnd.github+json",MetaLimit));
        Version version=release!=null && release.tag_name!=null && release.tag_name.StartsWith("v",StringComparison.Ordinal) ?
            Parse(release.tag_name.Substring(1)):null;
        if(release==null || release.draft!=false || release.prerelease!=false || version==null) throw new InvalidDataException();
        result.latestVersion=Display(version);result.releaseNotes=Clip(release.body,12000);
        result.releaseUrl="https://github.com/"+repository+"/releases/tag/"+release.tag_name;
        result.state=version>current?"available":"current";result.message=noticeMessage;
        if(version>current) {
            // A previously verified newer release takes precedence if the remote "latest" tag changes.
            if(result.readyVersion!=null && Parse(result.readyVersion)>=version) {result.state="ready";return;}
            if(prepare) {Stage(get,release,version,result);result.readyVersion=Display(version);result.state="ready";}
        }
    }
    static Snapshot Fail(Snapshot result,string state,string message) {result.state=state;result.message=message;return result;}
    internal string Execute(string command) {
        var result=Cached();
        if(command=="status") return json.Serialize(result);
        bool saveCheck=false;
        using(var mutex=new Mutex(false,lockName)) {
            bool owned;
            try {owned=mutex.WaitOne(0);}catch(AbandonedMutexException){owned=true;}
            if(!owned) return json.Serialize(Fail(result,"busy","另一个更新操作正在进行。"));
            try {
                result=Cached();
                if(command=="auto-on" || command=="auto-off") {
                    string path=Path.Combine(directory,"auto-off");
                    if(command=="auto-off") AtomicWrite(path,new byte[0]);
                    else if(File.Exists(path))File.Delete(path);
                    return json.Serialize(Cached());
                }
                if(command=="setAccess" || command=="clearAccess" || command=="useMachine") {
                    if(command=="setAccess" && !access.Prompt())return json.Serialize(result);
                    if(command!="setAccess") access.Switch(command=="clearAccess"?"off":"machine");
                    result=Cached();
                    result.state=result.readyVersion!=null?"ready":"idle";result.message=null;result.checkedAt=null;
                    Save("cache.json",result);
                    return json.Serialize(result);
                }
                if(command!="check" && command!="download" && command!="auto") throw new ArgumentException();
                if(command=="auto") {
                    if(!result.auto || result.accessMode=="off")return json.Serialize(result);
                }
                saveCheck=true;
                result.checkedAt=DateTime.UtcNow.ToString("o");
                string token=access.Token();
                if(token==null) Fail(result,"unauthorized","未获得私有仓库的读取权限。请设置 GitHub 访问方式。");
                else Refresh(result,new GithubReleaseApi(repository,token).Fetch,command!="check");
            } catch(Exception e) {
                if(Denied(e)) Fail(result,"unauthorized","无法读取私有仓库或正式版本，请检查 GitHub 权限。");
                else Fail(result,"error","检查或下载失败，请检查网络后重试。已下载的完整版本仍可使用。");
            } finally {
                // Only network operations share a persistent check result.
                try {if(saveCheck) Save("cache.json",result);}
                finally {mutex.ReleaseMutex();}
            }
        }
        return json.Serialize(result);
    }
    internal void RejectFolder(string folder) {AtomicWrite(Path.Combine(directory,folder,"rejected"),new byte[0]);}
    internal string PreparedLauncher(out string folder) {
        folder=null;
        foreach(string pointer in new[]{"ready.json","previous.json"}) {
            var ready=Read<Ready>(pointer);
            if(!ValidReady(ready) || !IsNew(ready.version)) continue;
            string dir=Path.Combine(directory,ready.folder),exe=Path.Combine(dir,FastName);
            if(File.Exists(Path.Combine(dir,"rejected"))) continue;
            try {
                if(!File.Exists(Path.Combine(dir,FullName)) || FileHash(exe)!=ready.fastHash ||
                    AssemblyName.GetAssemblyName(exe).Version!=Parse(ready.version)) {RejectFolder(ready.folder);continue;}
                folder=ready.folder;return exe;
            } catch {continue;}
        }
        return null;
    }
}

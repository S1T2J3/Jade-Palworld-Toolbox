using System;
using System.IO;
using System.Linq;
using System.Collections.Generic;
using System.Globalization;
using System.Security;
using System.Security.Cryptography;
using System.Text;
using System.Text.RegularExpressions;
using System.Web.Script.Serialization;
using Microsoft.Win32;

// Resolve the live world's level and player files. Never visit backup trees or
// global cross-world storage, and keep individual filenames inside the host.
internal static class TerminalSources
{
    const string HostId = "00000000000000000000000000000001";
    static readonly Regex LevelPart = new Regex(@"^Level(?:[-/\\](\d{1,4}))?$", RegexOptions.IgnoreCase | RegexOptions.CultureInvariant);
    static readonly Regex PlayerPart = new Regex(@"^Players[-/\\]([a-f0-9]{32}|[a-f0-9]{8}(?:-[a-f0-9]{4}){3}-[a-f0-9]{12})$", RegexOptions.IgnoreCase | RegexOptions.CultureInvariant);
    sealed class Player { public string id, path; }
    sealed class Files
    {
        readonly SortedDictionary<int,string> levels = new SortedDictionary<int,string>();
        readonly SortedDictionary<string,Player> players = new SortedDictionary<string,Player>(StringComparer.Ordinal);
        readonly SortedDictionary<string,string> stamps = new SortedDictionary<string,string>(StringComparer.OrdinalIgnoreCase);

        public void Add(string part, string path)
        {
            Match level = LevelPart.Match(part), player = PlayerPart.Match(part);
            if (!level.Success && !player.Success) return;
            FileInfo f = new FileInfo(path);
            CheckLocal(f.FullName);
            if (f.Length < 12) throw new InvalidDataException("部分终端存档尚未写入完整。请等待游戏保存后重试。");
            stamps.Add(part,f.FullName + "\0" + f.Length.ToString(CultureInfo.InvariantCulture) + "\0" + f.LastWriteTimeUtc.Ticks.ToString(CultureInfo.InvariantCulture));
            if (level.Success)
            {
                int index = level.Groups[1].Success ? Int32.Parse(level.Groups[1].Value,CultureInfo.InvariantCulture) : -1;
                if (levels.ContainsKey(index)) throw new InvalidDataException("世界文件分段重复，请重新检测当前存档。");
                levels.Add(index,f.FullName);
            }
            else
            {
                string id = Guid.Parse(player.Groups[1].Value).ToString("N");
                if (players.ContainsKey(id)) throw new InvalidDataException("玩家存档有多个版本。");
                players.Add(id,new Player { id=id,path=f.FullName });
            }
        }
        public object Manifest(bool local)
        {
            if (levels.Count==0) throw new InvalidDataException("此世界缺少 Level.sav。请在主机或服务器上选择完整的世界存档。");
            if (players.Count==0) throw new InvalidDataException("此世界缺少玩家存档。请在主机或服务器上选择含 Players 的完整世界存档。");
            string hash;
            using (var sha=SHA256.Create())
                hash=BitConverter.ToString(sha.ComputeHash(Encoding.UTF8.GetBytes(String.Join("\n",stamps.Select(p=>p.Key+"\0"+p.Value))))).Replace("-","");
            return new { level=levels.Values.ToArray(),players=players.Values.ToArray(), snapshot=hash,
                localOwner=local && players.ContainsKey(HostId) ? HostId : "" };
        }
    }
    static void CheckLocal(string path)
    {
        if ((File.GetAttributes(path) & (FileAttributes.ReparsePoint | FileAttributes.Offline))!=0)
            throw new InvalidDataException("部分游戏存档未保存在本机，或指向其他位置。请选择完整的本地存档目录。");
    }
    static bool SamePath(string a, string b) { return String.Equals(a,b,StringComparison.OrdinalIgnoreCase); }
    internal static bool OwnsLocalSteamAccount(string account, string[] localAccounts, uint activeUser)
    {
        // Steam's ActiveUser is the 32-bit account component of a public
        // individual SteamID64 (universe 1, type 1, desktop instance 1).
        if(activeUser!=0)
            return account==(76561197960265728UL+activeUser).ToString(CultureInfo.InvariantCulture);
        return localAccounts.Length==1 && localAccounts[0]==account;
    }
    static bool LocalAccount(DirectoryInfo account, bool xbox)
    {
        if(account==null || account.Parent==null) return false;
        try {
            if(xbox) {
                var valid=account.Parent.EnumerateDirectories().Where(d=>XboxSaves.IsAccount(d.Name) &&
                    File.Exists(Path.Combine(d.FullName,"containers.index"))).Take(2).ToArray();
                return valid.Length==1 && SamePath(valid[0].FullName,account.FullName);
            }
            var numeric=account.Parent.EnumerateDirectories().Select(d=>d.Name)
                .Where(n=>Regex.IsMatch(n,@"^\d{17}$")).ToArray();
            uint active=0;
            try {
                using(var key=Registry.CurrentUser.OpenSubKey(@"Software\Valve\Steam\ActiveProcess")) {
                    object value=key==null?null:key.GetValue("ActiveUser");
                    if(value is int) active=unchecked((uint)(int)value);
                }
            } catch(SecurityException) {} catch(UnauthorizedAccessException) {}
            return OwnsLocalSteamAccount(account.Name,numeric,active);
        } catch(IOException) {return false;} catch(UnauthorizedAccessException) {return false;} catch(SecurityException) {return false;}
    }
    internal static string Json(string directory,string id,bool xbox)
    {
        var json=new JavaScriptSerializer {MaxJsonLength=8*1024*1024};
        try {
            if (!Path.IsPathRooted(directory) || !Regex.IsMatch(id,@"^pal-[a-f0-9]{64}$")) throw new InvalidDataException("请选择重新检测到的游戏世界。");
            var dir=new DirectoryInfo(directory);
            CheckLocal(dir.FullName);
            var files=new Files();
            bool local;
            string appData=Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
            if(xbox) {
                XboxSaves.VisitTerminalParts(dir.FullName,(account,world)=>PalGameSaveDiscovery.MakeId("Xbox / Game Pass",account,world)==id,
                    part=>LevelPart.IsMatch(part)||PlayerPart.IsMatch(part), files.Add);
                local=dir.Parent!=null && dir.Parent.Parent!=null && SamePath(dir.Parent.Parent.FullName,
                    Path.Combine(appData,"Packages",XboxSaves.Package,"SystemAppData","wgs")) && LocalAccount(dir.Parent,true);
            } else {
                string platform,account,world;
                if (!PalGameSaveDiscovery.DescribeWorld(dir.FullName,out platform,out account,out world) ||
                    PalGameSaveDiscovery.MakeId(platform,account,world)!=id) throw new InvalidDataException("游戏世界路径已变更，请重新检测。");
                CheckLocal(Path.Combine(dir.FullName,"LevelMeta.sav"));
                foreach(var f in dir.EnumerateFiles("Level*.sav")) files.Add(Path.GetFileNameWithoutExtension(f.Name),f.FullName);
                string levelDir=Path.Combine(dir.FullName,"Level");
                if(Directory.Exists(levelDir)) {
                    CheckLocal(levelDir);
                    foreach(var f in new DirectoryInfo(levelDir).EnumerateFiles("*.sav")) files.Add("Level/"+Path.GetFileNameWithoutExtension(f.Name),f.FullName);
                }
                string playersDir=Path.Combine(dir.FullName,"Players");
                if(Directory.Exists(playersDir)) {
                    CheckLocal(playersDir);
                    foreach(var f in new DirectoryInfo(playersDir).EnumerateFiles("*.sav")) files.Add("Players/"+Path.GetFileNameWithoutExtension(f.Name),f.FullName);
                }
                local=platform=="Steam" && dir.Parent!=null && dir.Parent.Parent!=null &&
                    SamePath(dir.Parent.Parent.FullName,Path.Combine(appData,"Pal","Saved","SaveGames")) &&
                    File.Exists(Path.Combine(dir.FullName,"LocalData.sav")) && LocalAccount(dir.Parent,false);
            }
            return json.Serialize(files.Manifest(local));
        }
        catch(InvalidDataException ex) {return json.Serialize(new {error=ex.Message});}
        catch(Exception ex) {
            if (!(ex is IOException || ex is UnauthorizedAccessException || ex is SecurityException || ex is ArgumentException)) throw;
            return json.Serialize(new {error="部分世界或玩家存档无法读取。请等待保存或云同步完成，再重新检测。"});
        }
    }
}

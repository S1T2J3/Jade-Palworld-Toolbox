using System;
using System.Diagnostics;
using System.IO;
using System.IO.Compression;
using System.Reflection;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;
using System.Threading;
using System.Threading.Tasks;
using System.Windows.Forms;

[assembly: AssemblyTitle("Jade · 幻兽帕鲁工具箱")]
[assembly: AssemblyProduct("Jade · 幻兽帕鲁工具箱")]
[assembly: AssemblyDescription("离线帕鲁图鉴、配种、世界地图与物品查询")]
[assembly: AssemblyVersion("1.0.5.0")]

internal static class DesktopHost {
    const string Title="Jade · 幻兽帕鲁工具箱";
    const string CacheMarker=".pal-atlas-installed";
    // The private Gecko shell uses this exit code only for a validated, user-requested update.
    const int RestartForUpdate=78;
    const int WM_SETICON=0x80;
    [DllImport("kernel32.dll",CharSet=CharSet.Unicode)] static extern IntPtr GetModuleHandle(string module);
    [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern IntPtr LoadIcon(IntPtr module,IntPtr resource);
    [DllImport("user32.dll")] static extern bool SetForegroundWindow(IntPtr window);
    [DllImport("user32.dll")] static extern bool ShowWindowAsync(IntPtr window,int command);
    [DllImport("user32.dll")] static extern bool IsIconic(IntPtr window);
    [DllImport("user32.dll")] static extern IntPtr SendMessageTimeout(IntPtr window,int message,IntPtr wp,IntPtr lp,uint flags,uint timeout,out IntPtr result);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc proc,IntPtr parameter);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr window,out uint pid);
    [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int GetWindowText(IntPtr window,StringBuilder text,int size);
    delegate bool EnumProc(IntPtr window,IntPtr parameter);
    static Stream Resource(string name) {
        Stream local=Assembly.GetExecutingAssembly().GetManifestResourceStream(name);
#if FAST_LAUNCHER
        if(local==null && (name=="pal.engine.zip" || name=="pal.content.zip")) {
            lock(companionLock) if(companion==null) companion=FindPackage();
            return companion.GetManifestResourceStream(name);
        }
#endif
        return local;
    }
#if FAST_LAUNCHER
    static Assembly companion;
    static readonly object companionLock=new object();
    static Assembly FindPackage() {
        string expected,filename;
        using(var reader=new StreamReader(Resource("pal.companion"),Encoding.UTF8)) {
            expected=reader.ReadLine();filename=reader.ReadLine();
        }
        string directory=Path.GetDirectoryName(Application.ExecutablePath);
        string[] candidates={filename,UpdateService.FullName,"Jade 幻兽帕鲁工具箱.exe","帕鲁配种手册.exe"};
        foreach(string candidate in candidates) {
            string path=Path.Combine(directory,candidate);
            if(!File.Exists(path)) continue;
            using(var stream=new FileStream(path,FileMode.Open,FileAccess.Read,FileShare.Read))
            using(var hash=SHA256.Create()) {
                if(!String.Equals(Digest(hash.ComputeHash(stream)),expected,StringComparison.OrdinalIgnoreCase)) continue;
                // Runtime/code always comes from the verified application cache and this
                // launcher. The companion supplies embedded data and is never executed.
                return Assembly.ReflectionOnlyLoadFrom(path);
            }
        }
        throw new InvalidDataException("准备或修复此版本时需要同版本完整程序。请将「"+filename+"」和快速启动程序放在同一文件夹，再重新打开。");
    }
#endif
    static string Digest(byte[] bytes) {return BitConverter.ToString(bytes).Replace("-","").ToLowerInvariant();}
    static string Signature(string value) {using(var hash=SHA256.Create()) return Digest(hash.ComputeHash(Encoding.UTF8.GetBytes(value))).Substring(0,24);}
    static string Quote(string arg) {
        var value=new StringBuilder("\"");int slashes=0;
        foreach(char ch in arg) {
            if(ch=='\\'){slashes++;continue;}
            if(ch=='"'){value.Append('\\',slashes*2+1);value.Append('"');}
            else{value.Append('\\',slashes);value.Append(ch);}
            slashes=0;
        }
        value.Append('\\',slashes*2);return value.Append('"').ToString();
    }
    static void Take(Mutex mutex) {try{mutex.WaitOne();}catch(AbandonedMutexException){}}
    static bool TryTake(Mutex mutex) {try{return mutex.WaitOne(0);}catch(AbandonedMutexException){return true;}}
    static bool IsInstalled(string location,string digest) {
        string marker=Path.Combine(location,CacheMarker);
        return File.Exists(marker) && File.ReadAllText(marker)==digest;
    }
    static void Replace(string staged,string target) {
        if(File.Exists(target)) File.Replace(staged,target,null);
        else File.Move(staged,target);
    }
    static void Unpack(string resource,string digest,string folder,string existingContent=null) {
        string root=Path.GetFullPath(folder).TrimEnd(Path.DirectorySeparatorChar)+Path.DirectorySeparatorChar;
        using(Stream stream=Resource(resource)) {
            if(stream==null) throw new InvalidDataException("程序资源缺失。");
            using(var hash=SHA256.Create()) if(Digest(hash.ComputeHash(stream))!=digest) throw new InvalidDataException("程序包校验失败。请重新获取程序。");
            stream.Position=0;
            string lastDirectory=null;
            var buffer=new byte[81920];
            using(var archive=new ZipArchive(stream,ZipArchiveMode.Read)) foreach(var entry in archive.Entries) {
                string relative=entry.FullName.Replace('/',Path.DirectorySeparatorChar);
                string file=Path.GetFullPath(Path.Combine(root,relative));
                if(!file.StartsWith(root,StringComparison.OrdinalIgnoreCase) || entry.Name.Length==0) throw new InvalidDataException("无效的程序资源路径。");
                if(existingContent!=null && !relative.Equals("index.html",StringComparison.OrdinalIgnoreCase)) {
                    var installed=new FileInfo(Path.Combine(existingContent,relative));
                    if(installed.Exists && installed.Length==entry.Length) continue;
                }
                string directory=Path.GetDirectoryName(file);
                if(directory!=lastDirectory){Directory.CreateDirectory(directory);lastDirectory=directory;}
                using(var input=entry.Open()) using(var output=new FileStream(file,FileMode.CreateNew,FileAccess.Write,FileShare.None)) {
                    int count;
                    while((count=input.Read(buffer,0,buffer.Length))!=0) output.Write(buffer,0,count);
                    if(output.Position!=entry.Length) throw new InvalidDataException("程序资源未写入完整。");
                }
            }
        }
    }
    static string InstallEngine(string root,string digest,bool quiet) {
        string parent=Path.Combine(root,"runtimes"), target=Path.Combine(parent,digest.Substring(0,24));
        if(IsInstalled(target,digest) && File.Exists(Path.Combine(target,"firefox.exe")) && File.Exists(Path.Combine(target,"xul.dll"))) return target;
        Directory.CreateDirectory(parent);
        string stage=Path.Combine(parent,".stage-"+Guid.NewGuid().ToString("N"));Directory.CreateDirectory(stage);
        try {
            Unpack("pal.engine.zip",digest,stage);
            RuntimeBootstrap.Prepare(root,stage,quiet);
            File.WriteAllText(Path.Combine(stage,CacheMarker),digest);
            if(Directory.Exists(target)) Directory.Move(target,Path.Combine(parent,".incomplete-"+Guid.NewGuid().ToString("N")));
            Directory.Move(stage,target);return target;
        } finally {if(Directory.Exists(stage)) Directory.Delete(stage,true);}
    }
    static string InstallContent(string root,string digest) {
        string target=Path.Combine(root,"app"), index=Path.Combine(target,"index.html");
        if(IsInstalled(target,digest) && File.Exists(index)) return index;
        bool firstInstall=!Directory.Exists(target);
        string stage=Path.Combine(root,".content-"+Guid.NewGuid().ToString("N"));Directory.CreateDirectory(stage);
        try {
            Unpack("pal.content.zip",digest,stage,firstInstall?null:target);
            if(firstInstall) {
                File.WriteAllText(Path.Combine(stage,CacheMarker),digest);
                Directory.Move(stage,target);
                return index;
            }
            // Assets and data tables are content-addressed. Publish dependencies
            // first, leave old hashes accessible, and replace the entry point last.
            foreach(string source in Directory.GetFiles(stage,"*",SearchOption.AllDirectories)) {
                string relative=source.Substring(stage.Length+1);
                if(relative.Equals("index.html",StringComparison.OrdinalIgnoreCase)) continue;
                string dest=Path.Combine(target,relative);Directory.CreateDirectory(Path.GetDirectoryName(dest));
                Replace(source,dest);
            }
            Replace(Path.Combine(stage,"index.html"),index);
            string mark=Path.Combine(stage,CacheMarker);File.WriteAllText(mark,digest);Replace(mark,Path.Combine(target,CacheMarker));return index;
        } finally {if(Directory.Exists(stage)) Directory.Delete(stage,true);}
    }
    static IntPtr AppWindow(int launcherPid,string exe) {
        IntPtr result=IntPtr.Zero;
        EnumWindows(delegate(IntPtr w,IntPtr arg) {
            uint owner;GetWindowThreadProcessId(w,out owner);
            var title=new StringBuilder(128);GetWindowText(w,title,title.Capacity);
            if(title.ToString()!=Title)return true;
            if(owner!=(uint)launcherPid) try {
                using(var process=Process.GetProcessById((int)owner)) {
                    if(!String.Equals(process.MainModule.FileName,exe,StringComparison.OrdinalIgnoreCase))return true;
                }
            } catch {return true;}
            result=w;return false;
        },IntPtr.Zero);
        return result;
    }
    static int Run(string root,bool extractOnly,EventWaitHandle ready=null) {
        string[] hashes;
        using(var reader=new StreamReader(Resource("pal.package.hashes"))) hashes=reader.ReadToEnd().Trim().Split('\n');
        string instanceKey=@"Local\PalAtlasDesktop_"+Signature(root), engine, index;
        Directory.CreateDirectory(root);
        using(var signal=new EventWaitHandle(false,EventResetMode.AutoReset,instanceKey+"_activate"))
        using(var instance=new Mutex(false,instanceKey)) {
            if(!extractOnly && !TryTake(instance)){signal.Set();if(ready!=null)ready.Set();return 0;}
            try {
                using(var installing=new Mutex(false,instanceKey+"_install")) {
                    Take(installing);
                    try {
                        string engineHash=hashes[0].Trim(),contentHash=hashes[1].Trim();
                        // On a fresh install the runtime and handbook have independent
                        // staging directories. Keep sequential I/O on small machines.
                        Task<string> content=null;
                        if(Environment.ProcessorCount>=4 && !Directory.Exists(Path.Combine(root,"app")))
                            content=Task.Run(()=>InstallContent(root,contentHash));
                        try {engine=InstallEngine(root,engineHash,extractOnly);}
                        finally {if(content!=null) content.GetAwaiter().GetResult();}
                        index=content==null?InstallContent(root,contentHash):content.Result;
                    }
                    finally {installing.ReleaseMutex();}
                }
                if(extractOnly) return 0;
                string profile=Path.Combine(root,"profile");Directory.CreateDirectory(profile);
                var start=new ProcessStartInfo(Path.Combine(engine,"firefox.exe"),
                    "-wait-for-browser -no-remote -profile "+Quote(profile)+" -chrome chrome://palatlas/content/main.xhtml");
                start.WorkingDirectory=engine;start.UseShellExecute=false;
                start.EnvironmentVariables["PAL_ATLAS_SHELL"]=Path.Combine(engine,"palatlas-shell");
                start.EnvironmentVariables["PAL_ATLAS_PAGE"]=index;
                start.EnvironmentVariables["PAL_ATLAS_DATA"]=root;
                start.EnvironmentVariables["PAL_ATLAS_EARLY_WINDOW"]="1";
                start.EnvironmentVariables["PAL_ATLAS_LAUNCHER"]=Application.ExecutablePath;
                start.EnvironmentVariables["MOZ_CRASHREPORTER_DISABLE"]="1";
                start.EnvironmentVariables.Remove("MOZ_MARIONETTE");
                start.EnvironmentVariables.Remove("XUL_APP_FILE");
                using(var process=Process.Start(start)) {
                    bool initialized=false,activate=false;
                    while(!process.WaitForExit(initialized?350:120)) {
                        activate=signal.WaitOne(0)||activate;
                        if(initialized && !activate) continue;
                        IntPtr w=AppWindow(process.Id,start.FileName);
                        if(w==IntPtr.Zero) continue;
                        if(!initialized) {
                            if(ready!=null) ready.Set();
                            // Both launchers carry the same /win32icon group (32512).
                            // Load the shared Win32 handle only after the window exists.
                            IntPtr icon=LoadIcon(GetModuleHandle(null),new IntPtr(32512)),ignored;
                            if(icon!=IntPtr.Zero) {
                                SendMessageTimeout(w,WM_SETICON,IntPtr.Zero,icon,2,300,out ignored);
                                SendMessageTimeout(w,WM_SETICON,new IntPtr(1),icon,2,300,out ignored);
                            }
                        }
                        initialized=true;
                        if(activate){if(IsIconic(w))ShowWindowAsync(w,9);SetForegroundWindow(w);activate=false;}
                    }
                    if(process.ExitCode!=RestartForUpdate || !File.Exists(Path.Combine(root,"updates","has-prepared"))) return process.ExitCode;
                }
            } finally {if(!extractOnly)instance.ReleaseMutex();}
        }
        // Gecko has flushed the profile, every browser window has closed and the
        // instance mutex is now free. Use the ordinary activation and rollback path.
        return Launch(root);
    }
    static int Launch(string root) {
        if(File.Exists(Path.Combine(root,"updates","has-prepared"))) {
            try {
                var updater=UpdateService.For(root);
                string folder,exe;
                while((exe=updater.PreparedLauncher(out folder))!=null) {
                    string id=Guid.NewGuid().ToString("N");
                    using(var ready=new EventWaitHandle(false,EventResetMode.ManualReset,@"Local\JadeReady_"+id)) {
                        var start=new ProcessStartInfo(exe,"--use-prepared "+Quote(root)+" "+id){UseShellExecute=false};
                        using(var child=Process.Start(start)) {
                            while(!ready.WaitOne(100)) {
                                if(child.HasExited) break;
                            }
                            if(ready.WaitOne(0)) return 0;
                        }
                    }
                    updater.RejectFolder(folder);
                }
            } catch { /* Keep the supplied launcher available if update files cannot be read. */ }
        }
        return Run(root,false);
    }
    static string VerifyRestart(UpdateService updater,string version) {
        Version expected=UpdateService.Parse(version);
        string folder;
        string exe=updater.PreparedLauncher(out folder);
        if(expected==null || exe==null || AssemblyName.GetAssemblyName(exe).Version!=expected)
            return "{\"state\":\"error\",\"message\":\"待安装版本已变化或文件缺失，请重新检查更新。\"}";
        // PreparedLauncher verifies the small launcher's release digest. Its embedded
        // companion manifest authenticates the full package before closing the old GUI.
        var companion=Assembly.ReflectionOnlyLoadFrom(exe);
        using(var info=new StreamReader(companion.GetManifestResourceStream("pal.companion"),Encoding.UTF8))
        using(var input=File.OpenRead(Path.Combine(Path.GetDirectoryName(exe),UpdateService.FullName)))
        using(var sha=SHA256.Create()) {
            if(!String.Equals(info.ReadLine(),Digest(sha.ComputeHash(input)),StringComparison.OrdinalIgnoreCase)) {
                updater.RejectFolder(folder);
                return "{\"state\":\"error\",\"message\":\"更新文件校验失败，请重新下载。\"}";
            }
        }
        return "{\"state\":\"restarting\"}";
    }
    static int WriteReadResult(string[] args,int legacyLength,string json) {
        byte[] bytes=Encoding.UTF8.GetBytes(json);
        if(args.Length==legacyLength) {
            using(Stream output=Console.OpenStandardOutput()) output.Write(bytes,0,bytes.Length);
            return 0;
        }
        Guid request;
        if(args.Length!=legacyLength+2 || !Path.IsPathRooted(args[legacyLength]) ||
            !Guid.TryParseExact(args[legacyLength+1],"N",out request)) return 2;
        string dir=Path.Combine(Path.GetFullPath(args[legacyLength]),"ipc");
        Directory.CreateDirectory(dir);
        // If the window was terminated between the child's exit and consumption,
        // discard abandoned responses on a later scan; active reads have a 45s timeout.
        foreach(string file in Directory.EnumerateFiles(dir,"read-*.json")) {
            string name=Path.GetFileNameWithoutExtension(file);
            Guid stale;
            if(!Guid.TryParseExact(name.Substring(5),"N",out stale))continue;
            try {if(File.GetLastWriteTimeUtc(file)<DateTime.UtcNow.AddHours(-1))File.Delete(file);}
            catch(IOException){} catch(UnauthorizedAccessException){}
        }
        // The parent reads only after a successful process exit. CreateNew also
        // prevents a pre-existing result from being overwritten.
        using(var file=new FileStream(Path.Combine(dir,"read-"+request.ToString("N")+".json"),
            FileMode.CreateNew,FileAccess.Write,FileShare.None)) file.Write(bytes,0,bytes.Length);
        return 0;
    }
    [STAThread] static int Main(string[] args) {
        if((args.Length==3 || args.Length==4) && args[0]=="--release") {
            try {
                if(!Path.IsPathRooted(args[2]))return 2;
                string root=Path.GetFullPath(args[2]);
                Guid request=Guid.Empty;
                if(args.Length==4 && !Guid.TryParseExact(args[3],"N",out request))return 2;
                var updater=UpdateService.For(root);
                string response=args[1].StartsWith("verify-restart:",StringComparison.Ordinal)
                    ? VerifyRestart(updater,args[1].Substring("verify-restart:".Length)) : updater.Execute(args[1]);
                byte[] bytes=Encoding.UTF8.GetBytes(response);
                if(args.Length==4)UpdateService.AtomicWrite(Path.Combine(root,"updates","ipc",request.ToString("N")+".json"),bytes);
                else using(Stream stdout=Console.OpenStandardOutput()) stdout.Write(bytes,0,bytes.Length);
                return 0;
            } catch {return 1;}
        }
        if(args.Length==3 && args[0]=="--use-prepared") {
            Guid id;
            if(!Guid.TryParseExact(args[2],"N",out id) || !Path.IsPathRooted(args[1]))return 2;
            try {
                using(var ready=EventWaitHandle.OpenExisting(@"Local\JadeReady_"+id.ToString("N")))
                    return Run(Path.GetFullPath(args[1]),false,ready);
            } catch {return 1;}
        }
        if((args.Length==4 || args.Length==6) && args[0]=="--terminal-sources") {
            try {
                if(args[1]!="wgs" && args[1]!="folder") return 2;
                return WriteReadResult(args,4,TerminalSources.Json(args[3],args[2],args[1]=="wgs"));
            } catch {return 1;}
        }
        if(((args.Length==1 || args.Length==3) && args[0]=="--scan-game-saves") ||
            ((args.Length==2 || args.Length==4) && args[0]=="--scan-game-folder")) {
            try {
                string directory=args[0]=="--scan-game-folder"?args[1]:null;
                if(directory!=null && !Path.IsPathRooted(directory)) return 2;
                return WriteReadResult(args,directory==null?1:2,PalGameSaveDiscovery.ScanJson(directory));
            } catch {return 1;}
        }
        try {
            if(args.Length==1 && args[0]=="--open-legacy"){PalLegacyMigration.OpenLegacyWindow();return 0;}
            if(args.Length==2 && args[0]=="--open-link"){
                Uri url;if(!Uri.TryCreate(args[1],UriKind.Absolute,out url) || !(url.Scheme=="https" || url.Scheme=="http") || String.IsNullOrEmpty(url.Host))return 2;
                Process.Start(new ProcessStartInfo(url.AbsoluteUri){UseShellExecute=true});return 0;
            }
            if(args.Length==2 && args[0]=="--extract") return Run(Path.GetFullPath(args[1]),true);
            if(args.Length==2 && args[0]=="--data-dir") return Launch(Path.GetFullPath(args[1]));
            if(args.Length!=0) return 2;
            return Launch(Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),"PalAtlasDesktop"));
        } catch(Exception error) {
            MessageBox.Show(error.Message,Title,MessageBoxButtons.OK,MessageBoxIcon.Error);return 1;
        }
    }
}

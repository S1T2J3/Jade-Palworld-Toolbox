using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Drawing;
using System.IO;
using System.Net;
using System.Reflection;
using System.Security.Cryptography;
using System.Text.RegularExpressions;
using System.Threading.Tasks;
using System.Windows.Forms;

internal static class RuntimeBootstrap {
    internal const string FileName = "Firefox Setup 153.6.0esr.exe";
    const string DownloadUrl = "https://archive.mozilla.org/pub/firefox/releases/153.6.0esr/win64/zh-CN/Firefox%20Setup%20153.6.0esr.exe";
    static readonly string[] DownloadUrls = {
        "https://download-installer.cdn.mozilla.net/pub/firefox/releases/153.6.0esr/win64/zh-CN/Firefox%20Setup%20153.6.0esr.exe",
        DownloadUrl
    };
    const string InstallerHash = "2d41fd155a331b7400fe709fe73bb823e22c983b9d07f11cfe5e6d56ac55bdc1c0c418a7f81b163152b12e461b9385f17bed53510ba678aefaa1f21c87bb1927";
    const long InstallerLength = 90514512;

    static string Hex(byte[] hash) { return BitConverter.ToString(hash).Replace("-","").ToLowerInvariant(); }

    static Dictionary<string,string> ReadManifest(string stage) {
        var files = new Dictionary<string,string>(StringComparer.OrdinalIgnoreCase);
        foreach(string line in File.ReadAllLines(Path.Combine(stage,"notices","firefox-runtime.sha256"))) {
            Match row = Regex.Match(line,@"^([0-9a-f]{64})  ([A-Za-z0-9._ /-]+)$");
            if(!row.Success || row.Groups[2].Value.StartsWith("/") || row.Groups[2].Value.Contains(".."))
                throw new InvalidDataException("Mozilla 引擎文件清单错误。");
            files.Add(row.Groups[2].Value.Replace('/',Path.DirectorySeparatorChar),row.Groups[1].Value);
        }
        if(!files.ContainsKey("firefox.exe") || !files.ContainsKey("xul.dll")) throw new InvalidDataException("引擎清单不完整。");
        return files;
    }

    static bool Matches(string file,string expected) {
        using(var reader=File.OpenRead(file)) using(var hash=SHA256.Create()) return Hex(hash.ComputeHash(reader))==expected;
    }

    // Copy only files in the clean official package. Old shell code and user profiles
    // are outside this list, even when the cache contains multiple handbook versions.
    static bool TryReuse(string candidate,string stage,Dictionary<string,string> files) {
        string scratch=Path.Combine(stage,".runtime-copy");
        try {
            if(!Matches(Path.Combine(candidate,"firefox.exe"),files["firefox.exe"])) return false;
            foreach(var entry in files) {
                string target=Path.Combine(scratch,entry.Key);
                Directory.CreateDirectory(Path.GetDirectoryName(target));
                using(var reader=File.OpenRead(Path.Combine(candidate,entry.Key)))
                using(var writer=File.Create(target))
                using(var hash=SHA256.Create())
                using(var copied=new CryptoStream(writer,hash,CryptoStreamMode.Write)) {
                    reader.CopyTo(copied); copied.FlushFinalBlock();
                    if(Hex(hash.Hash)!=entry.Value) return false;
                }
            }
        } catch(IOException) { return false; }
          catch(UnauthorizedAccessException) { return false; }
        MoveFiles(scratch,stage,files);
        return true;
    }

    static void MoveFiles(string source,string target,Dictionary<string,string> files) {
        foreach(string name in files.Keys) {
            string destination=Path.Combine(target,name);
            Directory.CreateDirectory(Path.GetDirectoryName(destination));
            File.Move(Path.Combine(source,name),destination);
        }
    }

    static void DownloadFrom(string url,string path,Action<string,int> progress) {
        ServicePointManager.SecurityProtocol|=SecurityProtocolType.Tls12;
        var request=(HttpWebRequest)WebRequest.Create(url);
        request.Timeout=30000; request.ReadWriteTimeout=30000;
        request.UserAgent="Jade-Palworld-Toolbox"; request.AllowAutoRedirect=false;
        using(var response=(HttpWebResponse)request.GetResponse())
        using(var input=response.GetResponseStream())
        using(var output=File.Create(path)) {
            if(response.StatusCode!=HttpStatusCode.OK || (response.ContentLength>=0 && response.ContentLength!=InstallerLength))
                throw new InvalidDataException("Mozilla 返回的安装包大小不符。");
            var buffer=new byte[81920]; int count,previous=-1;
            while((count=input.Read(buffer,0,buffer.Length))>0) {
                if(output.Length+count>InstallerLength) throw new InvalidDataException("安装包超过预期大小。");
                output.Write(buffer,0,count);
                int percent=(int)(100*output.Length/InstallerLength);
                if(percent!=previous) { previous=percent;progress("正在从 Mozilla 下载桌面引擎",percent); }
            }
        }
    }

    internal static bool ValidInstaller(string path) {
        using(var input=File.OpenRead(path)) using(var digest=SHA512.Create())
            return input.Length==InstallerLength && Hex(digest.ComputeHash(input))==InstallerHash;
    }

    static void Download(string path,Action<string,int> progress) {
        Exception failure=null;
        for(int i=0;i<DownloadUrls.Length;i++) {
            try {
                progress("正在连接引擎下载源 "+(i+1)+"/"+DownloadUrls.Length,-1);
                DownloadFrom(DownloadUrls[i],path,progress);
                if(!ValidInstaller(path)) throw new InvalidDataException("引擎安装包校验失败。");
                return;
            } catch(WebException e) {failure=e;}
              catch(IOException e) {failure=e;}
              catch(InvalidDataException e) {failure=e;}
            if(File.Exists(path)) File.Delete(path);
        }
        throw new IOException("无法下载桌面引擎。请使用带运行环境的完整安装包，或将官方的 "+FileName+" 放在工具箱旁后重试。",failure);
    }

    static void InstallOfficial(string stage,Dictionary<string,string> files,Action<string,int> progress) {
        string work=Path.Combine(stage,".official");
        Directory.CreateDirectory(work);
        string adjacent=Path.GetDirectoryName(Assembly.GetExecutingAssembly().Location);
        string local=Path.Combine(adjacent,FileName);
        if(!File.Exists(local)) local=Path.Combine(adjacent,"firefox-esr-x64.exe");
        string installer=Path.Combine(work,"firefox-setup.exe");
        if(File.Exists(local)) {
            progress("正在校验本地 Mozilla 安装包",-1);
            if(new FileInfo(local).Length!=InstallerLength) throw new InvalidDataException("同目录 Firefox 安装包的大小不符。");
            File.Copy(local,installer);
        } else {
            progress("正在连接 Mozilla",-1);
            try {Download(installer,progress);}
            catch(WebException e) {throw new IOException("无法获取桌面引擎。请改用带运行环境的完整安装包。",e);}
        }
        using(var input=File.OpenRead(installer)) using(var digest=SHA512.Create()) {
            if(input.Length!=InstallerLength || Hex(digest.ComputeHash(input))!=InstallerHash)
                throw new InvalidDataException("Mozilla 安装包的 SHA-512 校验失败。请重新获取官方安装包。");
            progress("正在解压桌面引擎",-1);
            // Mozilla's stub treats the complete tail (including spaces) as one raw directory name.
            string extracted=Path.Combine(work,"extracted");
            Directory.CreateDirectory(extracted);
            var start=new ProcessStartInfo(installer,"/ExtractDir="+extracted) {
                UseShellExecute=false,CreateNoWindow=true,WindowStyle=ProcessWindowStyle.Hidden,WorkingDirectory=work
            };
            using(var unpack=Process.Start(start)) {
                if(!unpack.WaitForExit(300000)) {unpack.Kill();unpack.WaitForExit();throw new IOException("Mozilla 安装包解压超时。");}
                if(unpack.ExitCode!=0) throw new IOException("Mozilla 安装包未能解压。");
            }
            // The signed installer is pinned byte for byte and writes into a fresh directory.
            MoveFiles(Path.Combine(extracted,"core"),stage,files);
        }
        Directory.Delete(work,true);
    }

    internal static void Prepare(string root,string stage,bool quiet) {
        var files=ReadManifest(stage);
        foreach(string dir in Directory.GetDirectories(Path.Combine(root,"runtimes"))) {
            if(!Regex.IsMatch(Path.GetFileName(dir),"^[a-f0-9]{24}$")) continue;
            bool present=TryReuse(dir,stage,files);
            string scratch=Path.Combine(stage,".runtime-copy");
            if(Directory.Exists(scratch)) Directory.Delete(scratch,true);
            if(present) return;
        }
        if(quiet || !Environment.UserInteractive) {InstallOfficial(stage,files,(s,n)=>{});return;}
        using(var form=new Form {Text="Jade · 准备桌面引擎",StartPosition=FormStartPosition.CenterScreen,
            FormBorderStyle=FormBorderStyle.FixedDialog,ControlBox=false,ClientSize=new Size(400,82)}) {
            var label=new Label {Left=18,Top=12,Width=365,Height=23,Text="正在准备桌面引擎"};
            var bar=new ProgressBar {Left=18,Top=43,Width=365,Height=17,Style=ProgressBarStyle.Marquee};
            form.Controls.Add(label);form.Controls.Add(bar);
            Exception failure=null;
            form.Shown+=async (sender,args)=>{
                try {await Task.Run(()=>InstallOfficial(stage,files,(message,percent)=>{
                    form.BeginInvoke(new Action(()=>{
                        label.Text=message+(percent<0?"":"  "+percent+"%");
                        bar.Style=percent<0?ProgressBarStyle.Marquee:ProgressBarStyle.Continuous;
                        if(percent>=0) bar.Value=percent;
                    }));
                }));} catch(Exception e) {failure=e;}
                form.Close();
            };
            form.ShowDialog();
            if(failure!=null) throw failure;
        }
    }
}

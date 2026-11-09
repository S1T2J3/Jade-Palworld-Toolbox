using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Drawing;
using System.IO;
using System.Net;
using System.Security.Cryptography;
using System.Text;
using System.Threading.Tasks;
using System.Windows.Forms;

internal sealed class ReleaseAccess {
    readonly string directory;
    readonly string repository;
    readonly string modePath;
    readonly string secretPath;
    internal ReleaseAccess(string directory,string repository) {
        this.directory=directory;
        this.repository=repository;
        modePath=Path.Combine(directory,"access.txt");
        secretPath=Path.Combine(directory,"credential.bin");
    }
    internal string Mode {
        get {
            if(!File.Exists(modePath)) return "machine";
            string value=File.ReadAllText(modePath);
            return value=="machine" || value=="stored" ? value : "off";
        }
    }
    internal bool HasStoredCredential { get {return File.Exists(secretPath);} }
    internal void Switch(string mode) {
        UpdateService.AtomicWrite(modePath,Encoding.ASCII.GetBytes(mode));
        if(File.Exists(secretPath)) File.Delete(secretPath);
    }
    internal bool Prompt() {
        using(var form=new Form {Text="Jade · GitHub 仓库访问",StartPosition=FormStartPosition.CenterScreen,
            TopMost=true,FormBorderStyle=FormBorderStyle.FixedDialog,MaximizeBox=false,MinimizeBox=false,
            AutoScaleMode=AutoScaleMode.Font,ClientSize=new Size(480,156)}) {
            var label=new Label {Left=16,Top=12,Width=450,Height=54,
                Text="请输入仅授权 "+repository+" 的细粒度访问令牌。\r\n仓库 Contents 权限选择只读。令牌将由当前 Windows 用户加密保存。"};
            var input=new TextBox {Left=16,Top=69,Width=448,UseSystemPasswordChar=true,MaxLength=255};
            var ok=new Button {Text="保存",Left=304,Top=110,Width=76,DialogResult=DialogResult.OK,Enabled=false};
            var cancel=new Button {Text="取消",Left=388,Top=110,Width=76,DialogResult=DialogResult.Cancel};
            input.TextChanged+=(s,e)=>{ok.Enabled=ValidToken(input.Text.Trim());};
            form.Controls.AddRange(new Control[]{label,input,ok,cancel});
            form.AcceptButton=ok;form.CancelButton=cancel;form.ActiveControl=input;
            if(form.ShowDialog()!=DialogResult.OK) return false;
            byte[] plain=Encoding.UTF8.GetBytes(input.Text.Trim());
            input.Clear();
            try {
                byte[] encrypted=ProtectedData.Protect(plain,Encoding.UTF8.GetBytes(repository),DataProtectionScope.CurrentUser);
                Directory.CreateDirectory(directory);
                UpdateService.AtomicWrite(secretPath,encrypted);
                UpdateService.AtomicWrite(modePath,Encoding.ASCII.GetBytes("stored"));
                return true;
            } finally {Array.Clear(plain,0,plain.Length);}
        }
    }
    static bool ValidToken(string value) {
        if(String.IsNullOrEmpty(value) || value.Length>255) return false;
        foreach(char c in value) if(c<=32 || c>126) return false;
        return true;
    }
    static string Helper(string executable,string arguments,string input,string candidate=null) {
        if(!File.Exists(executable)) return null;
        try {
            var start=new ProcessStartInfo(executable,arguments){UseShellExecute=false,CreateNoWindow=true,
                RedirectStandardOutput=true,RedirectStandardInput=true,RedirectStandardError=true,
                WorkingDirectory=Path.GetDirectoryName(executable)};
            start.EnvironmentVariables["GIT_TERMINAL_PROMPT"]="0";
            start.EnvironmentVariables["GCM_INTERACTIVE"]="Never";
            start.EnvironmentVariables["GH_PROMPT_DISABLED"]="1";
            if(candidate!=null) {
                start.EnvironmentVariables["JADE_SIGNER_CANDIDATE"]=candidate;
                start.EnvironmentVariables["PSModulePath"]=Path.Combine(Environment.SystemDirectory,@"WindowsPowerShell\v1.0\Modules");
            }
            using(var process=Process.Start(start)) {
                Task<string> stdout=process.StandardOutput.ReadToEndAsync(),stderr=process.StandardError.ReadToEndAsync();
                if(input!=null) process.StandardInput.Write(input);
                process.StandardInput.Close();
                if(!process.WaitForExit(candidate==null?5000:10000)){process.Kill();return null;}
                if(process.ExitCode!=0 || !Task.WaitAll(new Task[]{stdout,stderr},1500)) return null;
                return stdout.Result;
            }
        } catch {return null;}
    }
    static IEnumerable<string> GhLocations(string local,string programFiles) {
        yield return Path.Combine(local,"PalAtlasDesktop","tools","github-cli","gh.exe");
        yield return Path.Combine(programFiles,"GitHub CLI","gh.exe");
        yield return Path.Combine(local,"Programs","GitHub CLI","gh.exe");
        string current=Path.GetFullPath(Environment.CurrentDirectory).TrimEnd('\\');
        foreach(string part in (Environment.GetEnvironmentVariable("PATH")??"").Split(';')) {
            string dir=part.Trim().Trim('"'),absolute=null;
            // Skip relative, current-directory and network-share PATH entries.
            if(dir.Length<3 || dir[1]!=':' || dir[2]!='\\') continue;
            try {absolute=Path.GetFullPath(dir).TrimEnd('\\');}catch{continue;}
            if(!String.Equals(absolute,current,StringComparison.OrdinalIgnoreCase)) yield return Path.Combine(absolute,"gh.exe");
        }
    }
    static string GhToken(string executable) {
        if(!File.Exists(executable))return null;
        try {
            using(var locked=new FileStream(executable,FileMode.Open,FileAccess.Read,FileShare.Read)) {
                string powershell=Path.Combine(Environment.SystemDirectory,@"WindowsPowerShell\v1.0\powershell.exe");
                const string verify="-NoProfile -NonInteractive -Command \"$ErrorActionPreference='Stop'; Import-Module ($PSHOME+'\\Modules\\Microsoft.PowerShell.Security\\Microsoft.PowerShell.Security.psd1'); $s=Get-AuthenticodeSignature -LiteralPath $env:JADE_SIGNER_CANDIDATE; if ($s.Status -eq 'Valid' -and $s.SignerCertificate.GetNameInfo('SimpleName',$false) -ceq 'GitHub, Inc.') { 'verified' }\"";
                string signer=Helper(powershell,verify,null,executable);
                if(signer==null || signer.Trim()!="verified")return null;
                string result=Helper(executable,"auth token --hostname github.com",null);
                return result!=null && ValidToken(result.Trim())?result.Trim():null;
            }
        } catch {return null;}
    }
    internal string Token() {
        if(Mode=="off") return null;
        if(Mode=="stored") {
            if(!File.Exists(secretPath) || new FileInfo(secretPath).Length>8192) return null;
            byte[] bytes=ProtectedData.Unprotect(File.ReadAllBytes(secretPath),Encoding.UTF8.GetBytes(repository),DataProtectionScope.CurrentUser);
            try {
                string token=Encoding.UTF8.GetString(bytes);
                return ValidToken(token)?token:null;
            } finally {Array.Clear(bytes,0,bytes.Length);}
        }
        string programFiles=Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles);
        string local=Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        var seen=new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        foreach(string exe in GhLocations(local,programFiles)) {
            if(!seen.Add(exe))continue;
            string token=GhToken(exe);
            if(token!=null)return token;
        }
        foreach(string git in new[]{Path.Combine(programFiles,"Git"),Path.Combine(local,"Programs","Git")}) {
            string exe=Path.Combine(git,"mingw64","bin","git-credential-manager.exe");
            string result=Helper(exe,"get","protocol=https\nhost=github.com\npath="+repository+".git\n\n");
            if(result!=null) foreach(string line in result.Split('\n')) {
                if(!line.StartsWith("password=",StringComparison.Ordinal)) continue;
                string token=line.Substring(9).Trim();
                if(ValidToken(token)) return token;
            }
        }
        return null;
    }
}

internal sealed class GithubReleaseApi {
    readonly string repository,token;
    internal GithubReleaseApi(string repository,string token) {this.repository=repository;this.token=token;}
    internal static bool AssetLocation(Uri url) {
        return url!=null && url.IsAbsoluteUri && url.Scheme=="https" && url.IsDefaultPort &&
            url.UserInfo.Length==0 && (url.Host=="release-assets.githubusercontent.com" ||
            url.Host=="objects.githubusercontent.com" || url.Host=="github-releases.githubusercontent.com");
    }
    internal HttpWebRequest CreateRequest(Uri url,string accept,bool initial) {
        var request=(HttpWebRequest)WebRequest.Create(url);
        request.AllowAutoRedirect=false;
        request.Timeout=20000;request.ReadWriteTimeout=30000;
        request.UserAgent="Jade-Palworld-Toolbox";
        request.Accept=accept;
        if(initial && url.Scheme=="https" && url.Host=="api.github.com" && url.IsDefaultPort && url.UserInfo.Length==0) {
            request.Headers["X-GitHub-Api-Version"]="2022-11-28";
            if(!String.IsNullOrEmpty(token))request.Headers["Authorization"]="Bearer "+token;
        }
        return request;
    }
    internal void Fetch(string path,string accept,long limit,Stream output,bool asset) {
        ServicePointManager.SecurityProtocol|=SecurityProtocolType.Tls12;
        Uri url=new Uri("https://api.github.com/repos/"+repository+"/"+path);
        for(int hop=0;hop<5;hop++) {
            var request=CreateRequest(url,accept,hop==0);
            using(var response=(HttpWebResponse)request.GetResponse()) {
                int code=(int)response.StatusCode;
                if(code>=300 && code<400) {
                    Uri next;
                    if(!asset || !Uri.TryCreate(url,response.Headers["Location"],out next) || !AssetLocation(next)) throw new InvalidDataException();
                    url=next;continue;
                }
                if(code!=200 || response.ContentLength>limit) throw new InvalidDataException();
                using(Stream source=response.GetResponseStream()) {
                    var buffer=new byte[65536];
                    long total=0;
                    int count;
                    while((count=source.Read(buffer,0,buffer.Length))>0) {
                        total+=count;
                        if(total>limit) throw new InvalidDataException();
                        output.Write(buffer,0,count);
                    }
                    if(response.ContentLength>=0 && total!=response.ContentLength) throw new EndOfStreamException();
                    return;
                }
            }
        }
        throw new InvalidDataException();
    }
}

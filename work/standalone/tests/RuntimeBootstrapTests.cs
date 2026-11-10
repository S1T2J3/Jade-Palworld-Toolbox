using System;using System.IO;using System.Net;using System.Reflection;using System.Threading.Tasks;using System.IO.Compression;
class BootstrapTests {
 static void Assert(bool ok,string message){if(!ok)throw new Exception(message);}
 static int Main(string[] args){try{
 string root=Path.GetFullPath(args[0]);Directory.CreateDirectory(root);string stage=Path.Combine(root,"stage");ZipFile.ExtractToDirectory(Path.GetFullPath(args[1]),stage);Directory.CreateDirectory(Path.Combine(root,"runtimes"));
 var urls=(string[])typeof(RuntimeBootstrap).GetField("DownloadUrls",BindingFlags.Static|BindingFlags.NonPublic).GetValue(null);
 for(int i=0;i<urls.Length;i++)urls[i]="http://127.0.0.1:1/blocked";
 RuntimeBootstrap.Prepare(root,stage,true);Assert(File.Exists(Path.Combine(stage,"xul.dll")),"offline bootstrap missing xul");Console.WriteLine("PASS fresh bootstrap with all online sources blocked; local official installer used.");
 string official=Path.Combine(AppDomain.CurrentDomain.BaseDirectory,"firefox-esr-x64.exe");Assert(RuntimeBootstrap.ValidInstaller(official),"official hash");
 string bad=Path.Combine(root,"bad.exe");File.WriteAllText(bad,"invalid");Assert(!RuntimeBootstrap.ValidInstaller(bad),"invalid installer accepted");
 using(var listener=new HttpListener()){listener.Prefixes.Add("http://127.0.0.1:18765/");listener.Start();int hits=0;
 var server=Task.Run(()=>{for(int i=0;i<2;i++){var c=listener.GetContext();hits++;if(i==0){c.Response.ContentLength64=1;c.Response.OutputStream.WriteByte(0);}else{c.Response.ContentLength64=new FileInfo(official).Length;using(var f=File.OpenRead(official))f.CopyTo(c.Response.OutputStream);}c.Response.Close();}});
 urls[0]="http://127.0.0.1:18765/bad";urls[1]="http://127.0.0.1:18765/good";
 string downloaded=Path.Combine(root,"download.exe");typeof(RuntimeBootstrap).GetMethod("Download",BindingFlags.Static|BindingFlags.NonPublic).Invoke(null,new object[]{downloaded,new Action<string,int>((s,n)=>{})});server.Wait();Assert(hits==2 && RuntimeBootstrap.ValidInstaller(downloaded),"fallback failed");Console.WriteLine("PASS first source invalid -> second source success; exact SHA-512 verified.");
 }
 return 0;}catch(Exception e){Console.Error.WriteLine(e);return 1;}}
}

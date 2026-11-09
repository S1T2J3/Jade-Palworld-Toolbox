using System;
using System.Collections.Generic;
using System.IO;
using System.IO.Compression;
using System.Net;
using System.Reflection;
using System.Security.Cryptography;
using System.Text;
using System.Web.Script.Serialization;

internal static class DeltaTests {
    static string Hash(byte[] bytes) {
        using(var sha=SHA256.Create())return BitConverter.ToString(sha.ComputeHash(bytes)).Replace("-","").ToLowerInvariant();
    }
    static void Assert(bool ok,string message){if(!ok)throw new Exception(message);}
    static UpdateService.Asset Asset(string name,int id,byte[] bytes) {
        return new UpdateService.Asset{name=name,id=id,state="uploaded",size=bytes.Length,digest="sha256:"+Hash(bytes)};
    }
    static byte[] Patch(byte[] target,bool invalidOffset) {
        using(var output=new MemoryStream()) {
            byte[] magic=Encoding.ASCII.GetBytes("JADEDLT1");
            output.Write(magic,0,magic.Length);
            using(var zip=new GZipStream(output,CompressionMode.Compress,true))
            using(var cmd=new BinaryWriter(zip)) {
                for(int i=0;i<2;i++) {
                    cmd.Write(-1);cmd.Write(target[0]);
                    cmd.Write(target.Length-1);cmd.Write(invalidOffset?target.Length:1);
                    cmd.Write(0);
                }
            }
            return output.ToArray();
        }
    }
    public static void Run(string scratch) {
        var json=new JavaScriptSerializer();
        byte[] official=File.ReadAllBytes(Assembly.GetExecutingAssembly().Location),old=(byte[])official.Clone();
        old[0]=0;
        var full=Asset(UpdateService.FullName,10,official);
        var fast=Asset(UpdateService.FastName,11,official);
        string patchName=ReleaseDelta.Prefix+Hash(old)+".jdp";
        foreach(string scenario in new[]{"delta","wrong-base","digest","range","interrupted"}) {
            string root=Path.Combine(scratch,scenario);
            Directory.CreateDirectory(root);
            string basePath=Path.Combine(root,"base.exe");
            File.WriteAllBytes(basePath,scenario=="wrong-base"?official:old);
            var service=new UpdateService(root,"Owner/Repo",new Version(1,0,0,0),basePath);
            byte[] patch=Patch(official,scenario=="range");
            var patchAsset=Asset(patchName,12,patch);
            var release=new UpdateService.Release{tag_name="v1.1.0",draft=false,prerelease=false,assets=new[]{full,fast,patchAsset}};
            bool interrupt=scenario=="interrupted";
            if(scenario=="digest")patch[0]^=1;
            var hits=new List<string>();
            UpdateService.Fetch fetch=delegate(string path,string type,long limit,Stream dest,bool asset) {
                byte[] data;
                if(path=="releases/latest")data=Encoding.UTF8.GetBytes(json.Serialize(release));
                else if(path=="contents/announcements.json")data=Encoding.UTF8.GetBytes("{\"announcements\":[]}");
                else {
                    hits.Add(path);
                    data=path=="releases/assets/12"?patch:official;
                    if(interrupt){dest.Write(data,0,1);throw new WebException("simulated transfer failure");}
                }
                dest.Write(data,0,data.Length);
            };
            var view=new UpdateService.Snapshot();
            if(interrupt) {
                try {service.Refresh(view,fetch,true);throw new Exception("interrupted patch accepted");}catch(WebException){}
                string none;
                Assert(service.PreparedLauncher(out none)==null && hits.Count==1 && hits[0]=="releases/assets/12","interruption triggered a full download");
                interrupt=false;hits.Clear();
            }
            service.Refresh(view,fetch,true);
            bool applied=scenario=="delta"||scenario=="interrupted";
            string folder;
            string launcher=service.PreparedLauncher(out folder);
            Assert(launcher!=null && view.state=="ready","missing staged update for "+scenario);
            Assert(view.downloadKind==(applied?"delta":"full"),"unexpected transfer mode for "+scenario);
            long expectedBytes=applied?patch.Length:official.Length*2L+(scenario=="wrong-base"?0:patch.Length);
            Assert(view.downloadBytes==expectedBytes,"incorrect byte count for "+scenario);
            Assert(hits.Count==(applied?1:scenario=="wrong-base"?2:3),"unexpected transfers for "+scenario);
            Assert(Hash(File.ReadAllBytes(launcher))==Hash(official) &&
                Hash(File.ReadAllBytes(Path.Combine(Path.GetDirectoryName(launcher),UpdateService.FullName)))==Hash(official),"reconstruction mismatch");
        }
        Console.WriteLine("Delta reconstruction, corrupt asset, wrong baseline, out-of-range copy and interrupted-download retry passed.");
    }
}

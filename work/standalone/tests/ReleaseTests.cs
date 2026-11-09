using System;
using System.IO;
using System.Reflection;
using System.Security.Cryptography;
using System.Text;
using System.Web.Script.Serialization;

// Compile together with UpdateService.cs and ReleaseAccess.cs. All paths below
// point inside the scratch directory supplied by run-release-tests.ps1.
[assembly:AssemblyVersion("1.1.0.0")]
internal static class ReleaseTests {
    static readonly JavaScriptSerializer Json=new JavaScriptSerializer();
    static void Assert(bool value,string message) {if(!value)throw new Exception(message);}
    static int Main(string[] args) {
        try {return Run(args);}
        catch(Exception e){Console.Error.WriteLine(e.GetType().Name+" "+e.Message+" "+e.StackTrace);return 1;}
    }
    static int Run(string[] args) {
        string root=args[0], profile=Path.Combine(root,"profile"), stable=Path.Combine(root,"app","index.html");
        Directory.CreateDirectory(profile);Directory.CreateDirectory(Path.GetDirectoryName(stable));
        File.WriteAllText(Path.Combine(profile,"world.json"),"synthetic world");File.WriteAllText(stable,"original entry");
        const string repo="Example/Jade-Toolbox";
        var updates=new UpdateService(root,repo,new Version(1,0,0,0));
        var same=new UpdateService(root,repo,new Version(1,1,0,0));
        byte[] exe=File.ReadAllBytes(Assembly.GetExecutingAssembly().Location);
        string digest;
        using(var hash=SHA256.Create())digest="sha256:"+BitConverter.ToString(hash.ComputeHash(exe)).Replace("-","").ToLowerInvariant();
        var release=new UpdateService.Release{tag_name="v1.1.0",prerelease=false,draft=false,body="说明",
            assets=new[]{new UpdateService.Asset{name=UpdateService.FullName,size=exe.Length,id=10,digest=digest,state="uploaded"},
            new UpdateService.Asset{name=UpdateService.FastName,size=exe.Length,id=11,digest=digest,state="uploaded"}}};
        bool corrupt=false,badNotices=false;
        int assetRequests=0;
        UpdateService.Fetch api=delegate(string path,string type,long limit,Stream output,bool asset) {
            byte[] data;
            if(path=="contents/announcements.json" && type=="application/vnd.github.raw+json")
                data=Encoding.UTF8.GetBytes(badNotices?"{":"{\"announcements\":[{\"id\":\"hello\",\"title\":\"公告\",\"body\":\"line 1\\nline 2\",\"publishedAt\":\"2026-11-09\",\"url\":\"https://example.org/link\"}]}");
            else if(path=="releases/latest") data=Encoding.UTF8.GetBytes(Json.Serialize(release));
            else if(asset && type=="application/octet-stream" && (path=="releases/assets/10" || path=="releases/assets/11")) {
                assetRequests++;data=(byte[])exe.Clone();if(corrupt && path.EndsWith("10"))data[100]^=1;
            } else throw new Exception("Unexpected request: "+path);
            Assert(data.Length<=limit,"size limit");output.Write(data,0,data.Length);
        };
        var view=new UpdateService.Snapshot();
        updates.Refresh(view,api,false);
        Assert(view.state=="available" && view.latestVersion=="1.1.0" && view.releaseNotes=="说明" && assetRequests==0,"discovery");
        Assert(view.announcements.Length==1 && view.announcements[0].url==null,"sanitization");
        badNotices=true;corrupt=true;
        try {updates.Refresh(view,api,true);throw new Exception("corrupt accepted");} catch(InvalidDataException){}
        string folder;Assert(updates.PreparedLauncher(out folder)==null,"partial update became ready");
        Assert(view.announcements.Length==1,"offline notices disappeared");
        corrupt=false;updates.Refresh(view,api,true);
        string prepared=updates.PreparedLauncher(out folder);
        Assert(view.readyVersion=="1.1.0" && prepared!=null && File.Exists(prepared),"stage");
        Assert(same.PreparedLauncher(out folder)==null,"self launch loop");
        int before=assetRequests;updates.Refresh(view,api,true);Assert(assetRequests==before,"duplicate transfer");
        var currentView=new UpdateService.Snapshot();same.Refresh(currentView,api,true);
        Assert(currentView.state=="current" && assetRequests==before,"same version");
        release.tag_name="v0.9.9";updates.Refresh(view,api,true);Assert(assetRequests==before,"downgrade");
        release.prerelease=true;try {updates.Refresh(view,api,true);throw new Exception("prerelease accepted");}catch(InvalidDataException){}
        File.WriteAllBytes(prepared,new byte[]{1,2});
        Assert(updates.PreparedLauncher(out folder)==null,"modified executable selected");
        var snapshot=Json.Deserialize<UpdateService.Snapshot>(updates.Execute("auto-off"));Assert(!snapshot.auto,"auto-off");
        snapshot=Json.Deserialize<UpdateService.Snapshot>(updates.Execute("clearAccess"));Assert(snapshot.accessMode=="off","clear");
        snapshot=Json.Deserialize<UpdateService.Snapshot>(updates.Execute("check"));Assert(snapshot.state=="unauthorized","private auth");
        snapshot=Json.Deserialize<UpdateService.Snapshot>(updates.Execute("useMachine"));Assert(snapshot.accessMode=="machine","machine");
        snapshot=Json.Deserialize<UpdateService.Snapshot>(updates.Execute("auto-on"));Assert(snapshot.auto,"auto-on");
        Assert(File.ReadAllText(Path.Combine(profile,"world.json"))=="synthetic world" && File.ReadAllText(stable)=="original entry","storage changed");
        Assert(GithubReleaseApi.AssetLocation(new Uri("https://release-assets.githubusercontent.com/blob")) &&
            !GithubReleaseApi.AssetLocation(new Uri("https://release-assets.githubusercontent.com.example.org/blob")) &&
            !GithubReleaseApi.AssetLocation(new Uri("http://release-assets.githubusercontent.com/blob")) &&
            !GithubReleaseApi.AssetLocation(new Uri("https://user@release-assets.githubusercontent.com/blob")),"redirect allowlist");
        Console.WriteLine("Passed release discovery, offline notices, private access modes, asset validation and rollback selection.");
        return 0;
    }
}

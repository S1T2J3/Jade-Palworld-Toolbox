using System;
using System.IO;
using System.IO.Compression;
using System.Text;

// Stream a publisher-generated patch. Each signed length describes a literal
// (negative) or a range in the old full executable (positive, then offset).
internal static class ReleaseDelta {
    internal const string Prefix="Jade-Delta-from-";
    static void Rebuild(BinaryReader patch,Stream baseline,string output,long expected,byte[] buffer) {
        using(var dest=new FileStream(output,FileMode.CreateNew,FileAccess.Write,FileShare.None)) {
            for(int operations=0;operations<=1000000;operations++) {
                int length=patch.ReadInt32();
                if(length==0) {
                    if(dest.Length!=expected)throw new InvalidDataException();
                    return;
                }
                long count=Math.Abs((long)length);
                if(count>expected-dest.Position)throw new InvalidDataException();
                Stream source=patch.BaseStream;
                if(length>0) {
                    int offset=patch.ReadInt32();
                    if(offset<0 || count>baseline.Length-offset)throw new InvalidDataException();
                    baseline.Position=offset;
                    source=baseline;
                }
                while(count>0) {
                    int read=source.Read(buffer,0,(int)Math.Min(count,buffer.Length));
                    if(read==0)throw new EndOfStreamException();
                    dest.Write(buffer,0,read);
                    count-=read;
                }
            }
            throw new InvalidDataException();
        }
    }
    internal static void Apply(Stream baseline,string file,string folder,long fullSize,long fastSize) {
        using(var raw=File.OpenRead(file))
        using(var header=new BinaryReader(raw,Encoding.UTF8,true)) {
            if(Encoding.ASCII.GetString(header.ReadBytes(8))!="JADEDLT1")throw new InvalidDataException();
            using(var gzip=new GZipStream(raw,CompressionMode.Decompress))
            using(var patch=new BinaryReader(gzip)) {
                byte[] buffer=new byte[65536];
                Rebuild(patch,baseline,Path.Combine(folder,UpdateService.FullName),fullSize,buffer);
                Rebuild(patch,baseline,Path.Combine(folder,UpdateService.FastName),fastSize,buffer);
                if(gzip.ReadByte()!=-1)throw new InvalidDataException();
            }
        }
    }
}

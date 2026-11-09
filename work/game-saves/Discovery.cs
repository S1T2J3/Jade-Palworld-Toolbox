using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.IO.Compression;
using System.Linq;
using System.Security.Cryptography;
using System.Text;
using System.Text.RegularExpressions;
using System.Web.Script.Serialization;
using Microsoft.Win32;

internal static class PalGameSaveDiscovery
{
    // This reader only opens LevelMeta. The much larger Level and Players saves
    // are unnecessary for identifying a world.
    private const int MaxMetaSize = 2 * 1024 * 1024;
    private static readonly StringComparer Paths = StringComparer.OrdinalIgnoreCase;
    private static readonly Regex Account = new Regex(@"^\d{1,20}$", RegexOptions.CultureInvariant);
    private static readonly Regex LibraryPath = new Regex("^\\s*\"path\"\\s*\"((?:\\\\.|[^\"\\\\])*)\"", RegexOptions.IgnoreCase | RegexOptions.CultureInvariant);

    private sealed class Scan
    {
        public readonly List<object> saves = new List<object>();
        public readonly List<object> pending = new List<object>();
        public readonly List<string> warnings = new List<string>();
        public readonly List<string> roots = new List<string>();
        private readonly HashSet<string> visited = new HashSet<string>(Paths);
        private readonly HashSet<string> ids = new HashSet<string>(StringComparer.Ordinal);
        private bool joinedOnly;
        private int packedBytes;

        public void Warn(string message)
        {
            if (warnings.Count < 64 && !warnings.Contains(message)) warnings.Add(message);
            else if (warnings.Count == 64) warnings.Add("还有其他目录或存档读取失败。可选择单个目录查看。");
        }
        public void Root(string path) { if (!roots.Contains(path, Paths)) roots.Add(path); }

        public void Add(string platform, string account, string world, string meta, string location, DateTime modified)
        {
            try
            {
                if (!visited.Add(Path.GetFullPath(meta))) return;
                string id = MakeId(platform, account, world);
                if (ids.Contains(id)) return;
                if (ids.Count >= 512) { Warn("本次最多列出 512 个世界。可选择单个账号或世界目录继续查找。"); return; }
                if ((File.GetAttributes(meta) & (FileAttributes.ReparsePoint | FileAttributes.Offline)) != 0)
                { Warn("元数据不在本机，或指向其他位置（" + location + "）。"); return; }
                byte[] bytes;
                using (FileStream file = new FileStream(meta, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete))
                using (BinaryReader reader = new BinaryReader(file))
                {
                    if (file.Length < 12 || file.Length > MaxMetaSize) throw Invalid();
                    bytes = reader.ReadBytes((int)file.Length);
                }
                string stamp = modified.ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ss.fffZ", CultureInfo.InvariantCulture);
                int offset;
                int rawLength;
                if (OodleContainer(bytes, out offset, out rawLength))
                {
                    if (bytes.Length - offset > MaxMetaSize - packedBytes)
                    { Warn("本次元数据总量已达上限。可选择单个账号或世界目录继续查找。"); return; }
                    packedBytes += bytes.Length - offset;
                    string compressed = Convert.ToBase64String(bytes, offset, bytes.Length - offset);
                    pending.Add(new { id = id, platform = platform, path = location, modified = stamp, compressed = compressed, rawLength = rawLength });
                }
                else
                {
                    string name = ReadWorldName(bytes);
                    saves.Add(new { id = id, name = name, platform = platform, path = location, modified = stamp });
                }
                ids.Add(id);
            }
            catch (Exception ex)
            {
                if (!Expected(ex)) throw;
                Warn("无法读取世界元数据（" + location + "）。文件可能正在保存、已经损坏或采用了尚未支持的格式。");
            }
        }

        private string[] Subdirs(string dir)
        {
            try { return Directory.GetDirectories(dir).Where(d => (File.GetAttributes(d) & FileAttributes.ReparsePoint) == 0).ToArray(); }
            catch (Exception ex) { if (!Expected(ex)) throw; Warn("无法访问目录（" + dir + "）。"); return new string[0]; }
        }

        private void WorldDir(string dir)
        {
            string meta = Path.Combine(dir, "LevelMeta.sav");
            if (!File.Exists(meta))
            {
                if (File.Exists(Path.Combine(dir, "LocalData.sav"))) joinedOnly = true;
                else if (File.Exists(Path.Combine(dir, "Level.sav")))
                    Warn("世界目录缺少 LevelMeta.sav（" + dir + "）。");
                return;
            }
            string platform, account, world;
            if (!DescribeWorld(dir, out platform, out account, out world)) return;
            try { Add(platform, account, world, meta, dir, File.GetLastWriteTimeUtc(meta)); }
            catch (Exception ex) { if (!Expected(ex)) throw; Warn("无法访问世界目录（" + dir + "）。"); }
        }

        private void AccountDir(string dir)
        {
            foreach (string child in Subdirs(dir))
                if (!BackupPart(Path.GetFileName(child)) && (IsWorld(child) || File.Exists(Path.Combine(child, "LevelMeta.sav")))) WorldDir(child);
        }

        public void SaveRoot(string dir)
        {
            if (!Directory.Exists(dir)) return;
            Root(dir);
            foreach (string child in Subdirs(dir))
            {
                string name = Path.GetFileName(child);
                if (Account.IsMatch(name)) AccountDir(child);
                else if (IsWorld(child)) WorldDir(child);
            }
        }

        public void Selected(string selected)
        {
            string path = new DirectoryInfo(Path.GetFullPath(selected)).FullName;
            if (path.Length > Path.GetPathRoot(path).Length) path = path.TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);
            if (File.Exists(path) && Paths.Equals(Path.GetFileName(path), "LevelMeta.sav")) path = Path.GetDirectoryName(path);
            if (!Directory.Exists(path)) { Warn("所选目录不存在或无法访问。"); return; }
            if (path.Split(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar).Any(BackupPart))
            { Warn("所选目录属于备份。请选取正在使用的世界目录或 SaveGames。"); return; }
            Root(path);
            if (Paths.Equals(Path.GetFileName(path), "wgs") || File.Exists(Path.Combine(path, "containers.index"))) return;
            if (File.Exists(Path.Combine(path, "LevelMeta.sav")) || IsWorld(path)) { WorldDir(path); return; }
            if (Account.IsMatch(Path.GetFileName(path))) { AccountDir(path); return; }
            foreach (string suffix in new[] { "SaveGames", Path.Combine("Saved", "SaveGames"), Path.Combine("Pal", "Saved", "SaveGames") })
            {
                string dir = Path.Combine(path, suffix);
                if (Directory.Exists(dir)) { SaveRoot(dir); return; }
            }
            // Also accepts moved SaveGames roots whose basename was changed.
            SaveRoot(path);
        }

        public string Json()
        {
            if (joinedOnly) Warn("部分联机世界只有本机玩家数据，缺少世界名称。可在主机或服务器上选择含 LevelMeta.sav 的目录。");
            return new JavaScriptSerializer { MaxJsonLength = Int32.MaxValue }.Serialize(new { saves = saves, pending = pending, warnings = warnings, roots = roots });
        }
    }

    public static string ScanJson(string selectedDirectory)
    {
        string appData = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        return ScanFromLocations(selectedDirectory, appData, String.IsNullOrEmpty(selectedDirectory) ? SteamLibraries() : new string[0]);
    }

    internal static string ScanFromLocations(string selectedDirectory, string appData, IEnumerable<string> libraries)
    {
        Scan scan = new Scan();
        try
        {
            string xboxDirectory = selectedDirectory;
            if (String.IsNullOrEmpty(selectedDirectory))
            {
                string steamRoot = Path.Combine(appData, "Pal", "Saved", "SaveGames");
                scan.Root(steamRoot);
                scan.SaveRoot(steamRoot);
                foreach (string library in libraries) scan.SaveRoot(Path.Combine(library, @"steamapps\common\PalServer\Pal\Saved\SaveGames"));
                xboxDirectory = Path.Combine(appData, "Packages", XboxSaves.Package, "SystemAppData", "wgs");
                scan.Root(xboxDirectory);
            }
            else scan.Selected(selectedDirectory);
            XboxSaves.Scan(xboxDirectory, delegate(string world, string meta, string accountDir, DateTime modified)
            {
                scan.Add("Xbox / Game Pass", Path.GetFileName(accountDir), world, meta, Path.GetDirectoryName(meta), modified);
            }, scan.Warn);
        }
        catch (Exception ex) { if (!Expected(ex)) throw; scan.Warn("访问存档路径失败。请重新选择可读取的存档目录。"); }
        return scan.Json();
    }

    private static IEnumerable<string> SteamLibraries()
    {
        var libraries = new HashSet<string>(Paths);
        string standard = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ProgramFilesX86), "Steam");
        libraries.Add(standard);
        try
        {
            using (RegistryKey key = Registry.CurrentUser.OpenSubKey(@"Software\Valve\Steam"))
            {
                string registered = key == null ? null : key.GetValue("SteamPath") as string;
                if (!String.IsNullOrEmpty(registered)) libraries.Add(Path.GetFullPath(registered));
            }
        }
        catch (Exception ex) { if (!Expected(ex)) throw; }
        foreach (string install in libraries.ToArray())
        {
            try
            {
                var config = new FileInfo(Path.Combine(install, "steamapps", "libraryfolders.vdf"));
                if (!config.Exists || config.Length > 1024 * 1024) continue;
                foreach (string line in File.ReadLines(config.FullName))
                {
                    Match match = LibraryPath.Match(line);
                    if (!match.Success) continue;
                    string lib = match.Groups[1].Value.Replace(@"\\", @"\").Replace("\\\"", "\"");
                    if (Path.IsPathRooted(lib)) libraries.Add(Path.GetFullPath(lib));
                }
            }
            catch (Exception ex) { if (!Expected(ex)) throw; }
        }
        return libraries;
    }

    private static bool IsWorld(string path)
    {
        Guid ignored;
        string name = Path.GetFileName(path);
        return (name.Length == 32 || name.Length == 36) && Guid.TryParse(name, out ignored);
    }

    private static bool BackupPart(string part)
    {
        return Paths.Equals(part, "backup") || Paths.Equals(part, "backups");
    }

    private static bool Expected(Exception e)
    {
        return e is IOException || e is InvalidDataException || e is UnauthorizedAccessException || e is ArgumentException
            || e is NotSupportedException || e is System.Security.SecurityException || e is OverflowException;
    }

    public static string MakeId(string platform, string account, string world)
    {
        string identity = String.Join("\0", new[] { platform, account, world }).ToUpperInvariant();
        using (var sha = SHA256.Create())
            return "pal-" + BitConverter.ToString(sha.ComputeHash(Encoding.UTF8.GetBytes(identity))).Replace("-", "").ToLowerInvariant();
    }

    internal static bool DescribeWorld(string dir, out string platform, out string account, out string world)
    {
        var directory = new DirectoryInfo(dir);
        var parent = directory.Parent;
        account = parent == null ? "" : parent.Name;
        platform = "Steam";
        Guid guid;
        world = Guid.TryParse(directory.Name, out guid) ? guid.ToString("N") : directory.Name;
        if (parent == null) return false;
        if (account == "0") { platform = "专用服务器"; account = parent.FullName; }
        else if (!Account.IsMatch(account)) { platform = "本地存档"; account = parent.FullName; }
        return true;
    }

    private static InvalidDataException Invalid() { return new InvalidDataException("无效或尚未支持的世界元数据"); }
    private static int Header(byte[] data)
    {
        if (data == null || data.Length < 12 || data.Length > MaxMetaSize) throw Invalid();
        int h = Encoding.ASCII.GetString(data, 8, 3) == "CNK" ? 12 : 0;
        if (data.Length < h + 12) throw Invalid();
        return h;
    }

    private static int BoundedSize(byte[] data, int offset)
    {
        int n = BitConverter.ToInt32(data, offset);
        if (n <= 0 || n > MaxMetaSize) throw Invalid();
        return n;
    }

    private static bool OodleContainer(byte[] bytes, out int offset, out int rawLength)
    {
        int h = Header(bytes);
        offset = h + 12;
        rawLength = 0;
        if (Encoding.ASCII.GetString(bytes, h + 8, 3) != "PlM") return false;
        if (bytes[h + 11] != (byte)'1' || BoundedSize(bytes, h + 4) != bytes.Length - offset) throw Invalid();
        rawLength = BoundedSize(bytes, h);
        return true;
    }

    private static byte[] Inflate(byte[] data, int start, int length, int expected)
    {
        if (length < 6 || (data[start] & 15) != 8 || data[start] >> 4 > 7 || (data[start + 1] & 32) != 0
            || ((data[start] * 256 + data[start + 1]) % 31) != 0) throw Invalid();
        byte[] raw = new byte[expected];
        using (var body = new MemoryStream(data, start + 2, length - 6, false))
        using (var z = new DeflateStream(body, CompressionMode.Decompress))
        {
            int offset = 0;
            while (offset < expected)
            {
                int count = z.Read(raw, offset, expected - offset);
                if (count == 0) throw Invalid();
                offset += count;
            }
            if (z.ReadByte() != -1) throw Invalid();
        }
        uint a = 1, b = 0;
        foreach (byte x in raw) { a = (a + x) % 65521; b = (b + a) % 65521; }
        int end = start + length - 4;
        uint checksum = ((uint)data[end] << 24) | ((uint)data[end + 1] << 16) | ((uint)data[end + 2] << 8) | data[end + 3];
        if ((b << 16 | a) != checksum) throw Invalid();
        return raw;
    }

    public static string ReadWorldName(byte[] bytes)
    {
        int h = Header(bytes);
        if (Encoding.ASCII.GetString(bytes, 0, 4) != "GVAS")
        {
            if (Encoding.ASCII.GetString(bytes, h + 8, 3) != "PlZ") throw Invalid();
            int size = BoundedSize(bytes, h), packed = BoundedSize(bytes, h + 4), start = h + 12;
            if (bytes[h + 11] == (byte)'1' && packed == bytes.Length - start) bytes = Inflate(bytes, start, packed, size);
            else if (bytes[h + 11] == (byte)'2')
            {
                bytes = Inflate(bytes, start, bytes.Length - start, packed);
                bytes = Inflate(bytes, 0, packed, size);
            }
            else throw Invalid();
        }
        using (var r = new BinaryReader(new MemoryStream(bytes, false)))
        {
            int end = bytes.Length;
            if (r.ReadUInt32() != 0x53415647 || r.ReadInt32() != 3) throw Invalid();
            Skip(r, 18, end);
            FString(r, end);
            if (r.ReadInt32() != 3) throw Invalid();
            int versions = r.ReadInt32();
            if (versions < 0 || versions > 4096) throw Invalid();
            Skip(r, versions * 20, end);
            if (FString(r, end) != "/Script/Pal.PalWorldBaseInfoSaveGame") throw Invalid();
            return Tags(r, end, false);
        }
    }

    private static void Skip(BinaryReader r, long count, long end)
    {
        if (count < 0 || count > end - r.BaseStream.Position) throw Invalid();
        r.BaseStream.Position += count;
    }

    private static string FString(BinaryReader r, long end, bool playerText = false)
    {
        if (end - r.BaseStream.Position < 4) throw Invalid();
        int length = r.ReadInt32();
        if (length == 0) return "";
        bool wide = length < 0;
        int count = checked(wide ? -2 * length : length);
        int nul = wide ? 2 : 1;
        if (count < nul || count > 65536 || count > end - r.BaseStream.Position) throw Invalid();
        byte[] text = r.ReadBytes(count);
        if (text[count - 1] != 0 || (wide && text[count - 2] != 0)) throw Invalid();
        // The game's name length limit can split the final surrogate pair.
        // Show a replacement glyph for that invalid unit, as a text renderer does.
        Encoding encoding = wide ? (Encoding)new UnicodeEncoding(false, false, !playerText) : new UTF8Encoding(false, true);
        return encoding.GetString(text, 0, count - nul);
    }

    private static string Tags(BinaryReader r, long end, bool inside)
    {
        while (r.BaseStream.Position < end)
        {
            string name = FString(r, end);
            if (name == "None") break;
            string type = FString(r, end);
            long size = r.ReadInt64();
            string structName = null;
            if (type == "StructProperty") { structName = FString(r, end); Skip(r, 16, end); }
            else if (type == "BoolProperty") Skip(r, 1, end);
            else if (type == "MapProperty") { FString(r, end); FString(r, end); }
            else if (type == "ArrayProperty" || type == "SetProperty" || type == "ByteProperty" || type == "EnumProperty") FString(r, end);
            byte flag = r.ReadByte();
            if (flag == 1) Skip(r, 16, end);
            else if (flag != 0) throw Invalid();
            long start = r.BaseStream.Position;
            if (size < 0 || size > end - start) throw Invalid();
            long next = start + size;
            if (!inside && name == "SaveData" && structName == "PalWorldBaseInfoSaveData") return Tags(r, next, true);
            if (inside && name == "WorldName" && type == "StrProperty")
            {
                string value = FString(r, next, true);
                if (r.BaseStream.Position != next || String.IsNullOrWhiteSpace(value) || value.Length > 256 || value.Any(Char.IsControl)) throw Invalid();
                return value;
            }
            r.BaseStream.Position = next;
        }
        throw Invalid();
    }
}

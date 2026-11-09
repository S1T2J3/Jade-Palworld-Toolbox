using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Security;
using System.Text;
using System.Text.RegularExpressions;

// Palworld PC Game Pass uses the WGS binary index and per-container manifests.
// Format references: https://github.com/Z1ni/XGP-save-extractor and
// https://github.com/deafdudecomputers/PalworldSaveTools/blob/main/src/palworld_xgp_import/gamepass_manager.py
internal static class XboxSaves
{
    internal const string Package = "PocketpairInc.Palworld_ad4psfrxyesvt";
    private const string IndexName = "containers.index";
    private static readonly Regex AccountFolder = new Regex(@"^[0-9a-f]{1,32}_[0-9a-f-]{1,64}$",
        RegexOptions.IgnoreCase | RegexOptions.CultureInvariant);
    private static readonly Regex WorldContainer = new Regex(
        @"^(?<world>[0-9a-f]{32}|[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})[-/\\](?<part>.+)$",
        RegexOptions.IgnoreCase | RegexOptions.CultureInvariant);
    private static readonly Encoding UnicodeStrict = new UnicodeEncoding(false, false, true);
    internal static bool IsAccount(string name) { return AccountFolder.IsMatch(name); }

    private sealed class Entry
    {
        public string World;
        public string Part;
        public string Folder;
        public byte Generation;
        public long Timestamp;
    }

    // Arguments delivered to the common LevelMeta reader:
    // logical world GUID, exact local blob, account directory, last saved UTC.
    public static void Scan(string directory, Action<string, string, string, DateTime> accept, Action<string> warning)
    {
        try
        {
            string root = String.IsNullOrEmpty(directory)
                ? Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
                    "Packages", Package, "SystemAppData", "wgs")
                : Path.GetFullPath(directory);
            if (Array.Exists(root.Split(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar), part =>
                part.Equals("backup", StringComparison.OrdinalIgnoreCase) || part.Equals("backups", StringComparison.OrdinalIgnoreCase))) return;
            if (File.Exists(root) && Path.GetFileName(root).Equals(IndexName, StringComparison.OrdinalIgnoreCase))
                root = Path.GetDirectoryName(root);
            if (!Directory.Exists(root)) return;
            var chosen = new DirectoryInfo(root);
            if (File.Exists(Path.Combine(root, IndexName))) { ReadProfile(chosen, null, accept, warning); return; }
            Guid selectedFolder;
            if (Guid.TryParseExact(chosen.Name, "N", out selectedFolder) && chosen.Parent != null &&
                File.Exists(Path.Combine(chosen.Parent.FullName, IndexName)))
            {
                ReadProfile(chosen.Parent, selectedFolder.ToString("N"), accept, warning);
                return;
            }

            // Only descend into the Palworld package's WGS directory or one chosen explicitly.
            if (!chosen.Name.Equals("wgs", StringComparison.OrdinalIgnoreCase))
            {
                string child = Path.Combine(root, "wgs");
                string packageChild = Path.Combine(root, "SystemAppData", "wgs");
                if (Directory.Exists(child)) root = child;
                else if (Directory.Exists(packageChild)) root = packageChild;
                else return;
            }
            foreach (DirectoryInfo account in new DirectoryInfo(root).EnumerateDirectories())
            {
                if (!AccountFolder.IsMatch(account.Name) || !File.Exists(Path.Combine(account.FullName, IndexName))) continue;
                try { ReadProfile(account, null, accept, warning); }
                catch (Exception ex) { if (!ReadError(ex)) throw; warning("部分 Xbox 游戏存档的索引暂时无法读取。"); }
            }
        }
        catch (Exception ex)
        {
            if (!ReadError(ex)) throw;
            warning("Xbox 游戏存档目录或索引暂时无法读取。");
        }
    }

    private static bool ReadError(Exception error)
    {
        return error is IOException || error is InvalidDataException || error is UnauthorizedAccessException ||
               error is SecurityException || error is ArgumentException;
    }

    private static bool Plain(string path)
    {
        return (File.GetAttributes(path) & (FileAttributes.ReparsePoint | FileAttributes.Offline)) == 0;
    }

    private static BinaryReader Open(string path)
    {
        if (!Plain(path)) throw new InvalidDataException();
        return new BinaryReader(new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete));
    }

    private static byte[] Exact(BinaryReader reader, int count)
    {
        if (count < 0 || count > reader.BaseStream.Length - reader.BaseStream.Position) throw new InvalidDataException();
        byte[] value = reader.ReadBytes(count);
        if (value.Length != count) throw new EndOfStreamException();
        return value;
    }

    private static string StringField(BinaryReader reader)
    {
        uint length = reader.ReadUInt32();
        if (length > 65535) throw new InvalidDataException();
        return UnicodeStrict.GetString(Exact(reader, (int)length * 2)).TrimEnd('\0');
    }

    private static List<Entry> ReadIndex(BinaryReader reader, string selectedContainer, bool includeTerminal = false)
    {
        if (reader.ReadUInt32() != 14) throw new InvalidDataException();
        uint count = reader.ReadUInt32();
        StringField(reader); // Optional package display name.
        string app = StringField(reader);
        string family = app.Split('!')[0];
        if (!family.Equals(Package, StringComparison.OrdinalIgnoreCase)) return new List<Entry>();
        reader.ReadInt64(); reader.ReadUInt32(); StringField(reader); reader.ReadInt64();
        // Three empty wstrings and all fixed fields need at least 57 bytes per entry.
        if (count > (reader.BaseStream.Length - reader.BaseStream.Position) / 57) throw new InvalidDataException();
        var entries = new List<Entry>();
        for (uint i = 0; i < count; i++)
        {
            string logical = StringField(reader);
            StringField(reader); StringField(reader);
            byte generation = reader.ReadByte();
            uint state = reader.ReadUInt32();
            string folder = new Guid(Exact(reader, 16)).ToString("N").ToUpperInvariant();
            long timestamp = reader.ReadInt64();
            reader.ReadInt64(); reader.ReadInt64();
            if (state == 3) continue; // Cloud deletion tombstone, possibly with stale bytes still on disk.
            Match match = WorldContainer.Match(logical);
            if (!match.Success) continue;
            string part = match.Groups["part"].Value;
            // Only keep live metadata and, for a manual selection, the live container identifying its world.
            if (!part.Equals("LevelMeta", StringComparison.OrdinalIgnoreCase) &&
                !(includeTerminal && (part.StartsWith("Level", StringComparison.OrdinalIgnoreCase) ||
                    part.StartsWith("Players-", StringComparison.OrdinalIgnoreCase) || part.StartsWith("Players/", StringComparison.OrdinalIgnoreCase))) &&
                (!String.Equals(folder, selectedContainer, StringComparison.OrdinalIgnoreCase) ||
                 part.StartsWith("Slot", StringComparison.OrdinalIgnoreCase))) continue;
            entries.Add(new Entry {
                World = new Guid(match.Groups["world"].Value).ToString("N").ToUpperInvariant(),
                Part = part, Folder = folder, Generation = generation, Timestamp = timestamp
            });
        }
        return entries;
    }

    private static void ReadProfile(DirectoryInfo account, string selectedContainer,
        Action<string, string, string, DateTime> accept, Action<string> warning)
    {
        if (!Plain(account.FullName)) return;
        List<Entry> entries;
        using (BinaryReader index = Open(Path.Combine(account.FullName, IndexName))) entries = ReadIndex(index, selectedContainer);
        string selectedWorld = null;
        if (selectedContainer != null)
        {
            Entry selected = entries.Find(e => String.Equals(e.Folder, selectedContainer, StringComparison.OrdinalIgnoreCase));
            if (selected == null) return;
            selectedWorld = selected.World;
        }
        var known = new Dictionary<string, Entry>(StringComparer.OrdinalIgnoreCase);
        var ambiguous = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        foreach (Entry e in entries)
        {
            if (!String.Equals(e.Part, "LevelMeta", StringComparison.OrdinalIgnoreCase) ||
                (selectedWorld != null && e.World != selectedWorld)) continue;
            Entry previous;
            if (known.TryGetValue(e.World, out previous) && (previous.Folder != e.Folder || previous.Generation != e.Generation))
                ambiguous.Add(e.World);
            else known[e.World] = e;
        }
        foreach (Entry e in known.Values)
        {
            try
            {
                if (ambiguous.Contains(e.World)) throw new InvalidDataException();
                string blob = FindBlob(account, e);
                DateTime date = File.GetLastWriteTimeUtc(blob);
                if (e.Timestamp > 0) try { date = DateTime.FromFileTimeUtc(e.Timestamp); } catch (ArgumentOutOfRangeException) { }
                accept(e.World, blob, account.FullName, date);
            }
            catch (Exception ex)
            {
                if (!ReadError(ex)) throw;
                warning("部分 Xbox 世界的元数据不完整或正在云同步，可在同步完成后刷新。");
            }
        }
    }

    internal static void VisitTerminalParts(string directory, Func<string,string,bool> verifyWorld,
        Func<string,bool> includesPart, Action<string,string> add)
    {
        var metadata = new DirectoryInfo(directory);
        var account = metadata.Parent;
        if (account == null || !Plain(account.FullName) || !Plain(metadata.FullName)) throw new InvalidDataException();
        List<Entry> entries;
        using (var index = Open(Path.Combine(account.FullName, IndexName))) entries = ReadIndex(index, metadata.Name, true);
        string world = null;
        foreach (var entry in entries)
            if (String.Equals(entry.Folder, metadata.Name, StringComparison.OrdinalIgnoreCase) &&
                String.Equals(entry.Part, "LevelMeta", StringComparison.OrdinalIgnoreCase) && verifyWorld(account.Name,entry.World))
                world = entry.World;
        if (world == null) throw new InvalidDataException("Xbox 世界存档已迁移或正在云同步，请重新检测。");
        var generations = new Dictionary<string,Entry>(StringComparer.OrdinalIgnoreCase);
        foreach (var entry in entries)
        {
            if (entry.World != world || !includesPart(entry.Part)) continue;
            Entry other;
            if (generations.TryGetValue(entry.Part, out other) &&
                (other.Folder != entry.Folder || other.Generation != entry.Generation))
                throw new InvalidDataException("Xbox 存档有多个同步版本，请等待游戏完成云同步后重试。");
            generations[entry.Part] = entry;
        }
        foreach (var pair in generations) add(pair.Key, FindBlob(account, pair.Value));
    }

    private static string FindBlob(DirectoryInfo account, Entry entry)
    {
        string folder = Path.Combine(account.FullName, entry.Folder);
        if (!Plain(folder)) throw new InvalidDataException();
        string manifest = Path.Combine(folder, "container." + entry.Generation.ToString(CultureInfo.InvariantCulture));
        Guid cloud = Guid.Empty, local = Guid.Empty;
        bool found = false;
        using (BinaryReader r = Open(manifest))
        {
            if (r.ReadUInt32() != 4) throw new InvalidDataException();
            uint count = r.ReadUInt32();
            if (count == 0 || count > (r.BaseStream.Length - r.BaseStream.Position) / 160) throw new InvalidDataException();
            for (uint i = 0; i < count; i++)
            {
                string name = UnicodeStrict.GetString(Exact(r, 128)).TrimEnd('\0');
                Guid previous = new Guid(Exact(r, 16)), current = new Guid(Exact(r, 16));
                if (count > 1 && !name.Equals("Data", StringComparison.OrdinalIgnoreCase)) continue;
                if (found) throw new InvalidDataException();
                found = true; cloud = previous; local = current;
            }
        }
        if (!found) throw new InvalidDataException();
        string live = Path.Combine(folder, local.ToString("N").ToUpperInvariant());
        string old = Path.Combine(folder, cloud.ToString("N").ToUpperInvariant());
        bool hasLive = local != Guid.Empty && File.Exists(live);
        bool hasOld = cloud != Guid.Empty && File.Exists(old);
        if (!hasLive && !hasOld || (hasLive && hasOld && local != cloud)) throw new InvalidDataException();
        string file = hasLive ? live : old;
        if (!Plain(file)) throw new InvalidDataException();
        return file;
    }
}

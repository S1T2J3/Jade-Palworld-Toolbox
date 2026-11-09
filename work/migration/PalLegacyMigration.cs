using System;
using System.ComponentModel;
using System.Diagnostics;
using System.IO;

// Keep the original file URL and browser preference. Worlds are exported by
// the old page itself so its browser remains the owner of its site storage.
public static class PalLegacyMigration
{
    public static void OpenLegacyWindow()
    {
        string local = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        string html = Path.Combine(local, "PalBreedingAtlas", "index.html");
        if (!File.Exists(html))
        {
            throw new FileNotFoundException(
                "这台电脑上找不到旧 Windows 版的手册页面。\n" +
                "请打开原来的帕鲁手册；如果以前用的是 HTML 版，请在原浏览器中打开原文件。" +
                "在「世界档案」里选择「全部世界」并导出备份。");
        }

        string pageUri = new Uri(html).AbsoluteUri + "#warehouse";
        string[] roots = {
            Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles),
            Environment.GetFolderPath(Environment.SpecialFolder.ProgramFilesX86),
            local
        };
        string[] browsers = { @"Microsoft\Edge\Application\msedge.exe", @"Google\Chrome\Application\chrome.exe" };
        foreach (string browser in browsers)
        {
            foreach (string root in roots)
            {
                if (String.IsNullOrEmpty(root)) continue;
                string program = Path.Combine(root, browser);
                if (!File.Exists(program)) continue;
                try
                {
                    Process.Start(new ProcessStartInfo(program,
                        "--app=\"" + pageUri + "\" --no-first-run --no-default-browser-check") {
                        UseShellExecute = false
                    });
                    return;
                }
                catch (Win32Exception) { /* Try the next browser used by the original launcher. */ }
            }
        }
        try
        {
            Process.Start(new ProcessStartInfo(html) { UseShellExecute = true });
        }
        catch (Win32Exception e)
        {
            throw new IOException("请在原来使用的浏览器中打开手册页面：\n" + html, e);
        }
    }
}

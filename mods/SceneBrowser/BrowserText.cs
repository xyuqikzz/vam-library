using System;
using System.IO;
using System.Text;

namespace VamLibrary.SceneBrowser
{
    public static class BrowserText
    {
        public static bool English;
        private static long stamp = -1, size = -1;
        private static readonly string[] Ids = { "author", "folders", "imported", "size-desc", "size-asc", "missing", "invalid", "root", "parent", "folder", "folder-error" };
        private static readonly string[] Chinese = { "作者 A–Z", "按文件夹浏览", "入库时间", "资源包大小（大到小）", "资源包大小（小到大）", "暂无入库记录，在软件中入库或刷新资源后自动更新。", "读取入库时间失败，请在软件中刷新资源。", "返回 AddonPackages 根目录", ".. / 返回上一级", "[文件夹] ", "没能读取文件夹，请重新打开场景选择器。" };
        private static readonly string[] EnglishText = { "Creator A–Z", "Browse by folder", "Import time", "Package size (largest first)", "Package size (smallest first)", "No import records yet. Add or refresh resources in the app.", "Could not read import times. Refresh resources in the app.", "Back to AddonPackages", ".. / Parent folder", "[Folder] ", "Could not read this folder. Reopen the scene browser." };
        public static string Get(string id)
        {
            int i = Array.IndexOf(Ids, id);
            return i < 0 ? id : (English ? EnglishText[i] : Chinese[i]);
        }
        public static string Mode(string label)
        {
            for (int i = 0; i < 5; i++)
                if (label == Ids[i] || label == Chinese[i] || label == EnglishText[i]) return Ids[i];
            return null;
        }
        public static bool IsImportStatus(string label)
        { return label == Chinese[5] || label == Chinese[6] || label == EnglishText[5] || label == EnglishText[6]; }
        public static void Refresh(string path)
        {
            try
            {
                var info = new FileInfo(path);
                if (!info.Exists) { English = false; stamp = size = -1; return; }
                if (stamp == info.LastWriteTimeUtc.Ticks && size == info.Length) return;
                if (info.Length > 4096) return;
                string[] lines = File.ReadAllText(path, Encoding.UTF8).Replace("\r\n", "\n").TrimEnd('\n').Split('\n');
                if (lines.Length != 2 || lines[0] != "# VAM Library app settings v1" ||
                    (lines[1] != "language\ten-US" && lines[1] != "language\tzh-CN")) return;
                English = lines[1] == "language\ten-US";
                stamp = info.LastWriteTimeUtc.Ticks; size = info.Length;
            }
            catch (IOException) { }
            catch (UnauthorizedAccessException) { }
        }
    }
}

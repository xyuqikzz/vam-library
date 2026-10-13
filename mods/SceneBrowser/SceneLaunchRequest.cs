using System;
using System.Globalization;
using System.IO;
using System.Text;

namespace VamLibrary.SceneBrowser
{
    // No Unity calls here: the wire contract is tested independently of the game.
    public sealed class SceneLaunchRequest
    {
        public const string Header = "# VAM Library scene launch v2";
        public const long TimeoutMs = 1800000;
        public const long HeartbeatMs = 15000;
        public string Id;
        public long Created;
        public string Scene;
        public static long Now { get { return (DateTime.UtcNow.Ticks - new DateTime(1970, 1, 1).Ticks) / TimeSpan.TicksPerMillisecond; } }

        public static SceneLaunchRequest Parse(string text, string id, long now, long modified)
        {
            string[] lines = text.Replace("\r\n", "\n").TrimEnd('\n').Split('\n');
            long created;
            if (!ValidId(id) || lines.Length != 4 || lines[0] != Header || lines[1] != id ||
                !long.TryParse(lines[2], NumberStyles.None, CultureInfo.InvariantCulture, out created) ||
                created > now + 5000 || created < now - TimeoutMs || !Fresh(modified, now) || !ValidScene(lines[3]))
                throw new InvalidDataException("invalid_request");
            return new SceneLaunchRequest { Id = id, Created = created, Scene = lines[3] };
        }

        public static long Milliseconds(DateTime value)
        {
            return (value.ToUniversalTime().Ticks - new DateTime(1970, 1, 1).Ticks) / TimeSpan.TicksPerMillisecond;
        }

        public static bool Fresh(long modified, long now) { return modified <= now + 5000 && modified >= now - HeartbeatMs; }

        public static void Accept(string requestPath)
        {
            string target = Path.ChangeExtension(requestPath, ".accepted");
            string temp = target + ".tmp";
            try { File.WriteAllText(temp, ""); File.Move(temp, target); }
            finally { if (File.Exists(temp)) File.Delete(temp); }
        }

        public static void Cleanup(string directory, long now)
        {
            if (!Directory.Exists(directory)) return;
            foreach (string file in Directory.GetFiles(directory))
            {
                string extension = Path.GetExtension(file);
                if (extension != ".request" && extension != ".result" && extension != ".accepted" && extension != ".tmp") continue;
                try
                {
                    var info = new FileInfo(file);
                    if ((info.Attributes & FileAttributes.ReparsePoint) != 0) continue;
                    long modified = Milliseconds(info.LastWriteTimeUtc);
                    if (extension == ".request" ? !Fresh(modified, now) : modified < now - TimeoutMs) File.Delete(file);
                }
                catch (IOException) { /* The app may finish or refresh this request concurrently. */ }
                catch (UnauthorizedAccessException) { /* Leave inaccessible files alone. */ }
            }
        }

        public static bool ValidId(string id)
        {
            if (String.IsNullOrEmpty(id) || id.Length > 80) return false;
            foreach (char c in id) if (!(c >= '0' && c <= '9') && !(c >= 'a' && c <= 'f') && c != '-') return false;
            return true;
        }

        public static bool ValidScene(string scene)
        {
            if (String.IsNullOrEmpty(scene) || scene.Length > 2048 || scene.IndexOf('\\') >= 0) return false;
            int colon = scene.IndexOf(":/");
            string path = scene;
            if (colon >= 0)
            {
                string uid = scene.Substring(0, colon);
                int firstDot = uid.IndexOf('.');
                int lastDot = uid.LastIndexOf('.');
                long version;
                if (firstDot <= 0 || lastDot <= firstDot + 1 ||
                    !long.TryParse(uid.Substring(lastDot + 1), NumberStyles.None, CultureInfo.InvariantCulture, out version)) return false;
                foreach (char c in uid) if (Char.IsControl(c) || "<>:\"/\\|?*".IndexOf(c) >= 0) return false;
                path = scene.Substring(colon + 2);
            }
            if (!path.StartsWith("Saves/scene/", StringComparison.OrdinalIgnoreCase) ||
                !path.EndsWith(".json", StringComparison.OrdinalIgnoreCase)) return false;
            foreach (string part in path.Split('/'))
            {
                if (part.Length == 0 || part == "." || part == ".." || part.Trim() != part || part.EndsWith(".")) return false;
                foreach (char c in part) if (Char.IsControl(c) || "<>:\"/\\|?*".IndexOf(c) >= 0) return false;
            }
            return true;
        }

        public static void Complete(string requestPath, string id, string state, string code, string detail = null)
        {
            if (!ValidId(id) || !File.Exists(requestPath)) return;
            string result = Path.ChangeExtension(requestPath, ".result");
            string temp = result + ".tmp";
            string text = Header + "\n" + id + "\n" + state + "\n" +
                (code ?? "").Replace('\r', ' ').Replace('\n', ' ') + "\n" +
                (detail ?? "").Replace('\r', ' ').Replace('\n', ' ') + "\n";
            try
            {
                if (!File.Exists(result))
                {
                    File.WriteAllText(temp, text, new UTF8Encoding(false));
                    // Each request owns a unique result; no existing files are replaced.
                    File.Move(temp, result);
                }
                File.Delete(requestPath);
            }
            finally { if (File.Exists(temp)) File.Delete(temp); }
        }
    }
}

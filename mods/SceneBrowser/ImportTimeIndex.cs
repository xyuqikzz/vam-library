using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;

namespace VamLibrary.SceneBrowser
{
    public sealed class ImportTimeIndex
    {
        public const string Header = "# VAM Library import times v1";
        private Dictionary<string, long> times = new Dictionary<string, long>(StringComparer.OrdinalIgnoreCase);
        private DateTime stamp = DateTime.MinValue;
        private long size = -1;
        public bool Available { get; private set; }
        public string Error { get; private set; }
        public int Count { get { return times.Count; } }

        public long? Find(string packageUid)
        {
            long value;
            return packageUid != null && times.TryGetValue(packageUid, out value) ? (long?)value : null;
        }

        public static Dictionary<string, long> Parse(string text)
        {
            string[] lines = text.Split('\n');
            if (lines.Length == 0 || lines[0].TrimEnd('\r') != Header) throw new FormatException("Unknown import-time format");
            var result = new Dictionary<string, long>(StringComparer.OrdinalIgnoreCase);
            for (int i = 1; i < lines.Length; i++)
            {
                string line = lines[i].TrimEnd('\r');
                if (line.Length == 0) continue;
                int split = line.IndexOf('\t');
                long value;
                if (split <= 0 || split != line.LastIndexOf('\t') ||
                    !long.TryParse(line.Substring(split + 1), NumberStyles.Integer, CultureInfo.InvariantCulture, out value) || value < 0)
                    throw new FormatException("Invalid import-time entry");
                string id = line.Substring(0, split);
                foreach (char character in id) if (char.IsControl(character)) throw new FormatException("Invalid package UID");
                long previous;
                if (!result.TryGetValue(id, out previous) || value < previous) result[id] = value;
            }
            return result;
        }

        public bool Refresh(string path, bool force)
        {
            try
            {
                var file = new FileInfo(path);
                if (!file.Exists)
                {
                    bool changed = Available || times.Count > 0;
                    times.Clear(); Available = false; Error = null;
                    stamp = DateTime.MinValue; size = -1;
                    return changed;
                }
                if (!force && Available && file.LastWriteTimeUtc == stamp && file.Length == size) return false;
                Dictionary<string, long> loaded;
                // Permit the app to atomically replace the snapshot during a read.
                using (var reader = new StreamReader(new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete)))
                    loaded = Parse(reader.ReadToEnd());
                // Only publish a complete, validated snapshot; a failed read retains the last good data.
                times = loaded; stamp = file.LastWriteTimeUtc; size = file.Length;
                Available = true; Error = null;
                return true;
            }
            catch (Exception error)
            {
                if (!(error is IOException) && !(error is UnauthorizedAccessException) && !(error is FormatException)) throw;
                Error = error.Message;
                return false;
            }
        }
    }
}

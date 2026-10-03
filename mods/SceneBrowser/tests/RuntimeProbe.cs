using System;
using System.IO;
using System.Reflection;

class RuntimeProbe
{
    static int Main(string[] args)
    {
        string root = args[0];
        AppDomain.CurrentDomain.AssemblyResolve += delegate(object sender, ResolveEventArgs e)
        {
            string name = new AssemblyName(e.Name).Name + ".dll";
            foreach (string directory in new[] { "VaM_Data/Managed", "BepInEx/core" })
            {
                string path = Path.Combine(Path.Combine(root, directory), name);
                if (File.Exists(path)) return Assembly.LoadFrom(path);
            }
            return null;
        };
        try
        {
            Console.WriteLine("Game Mono CLR: " + Environment.Version);
            var assembly = Assembly.LoadFrom(args[1]);
            foreach (Type type in assembly.GetTypes())
            {
                // Force resolving members as well as type names: the original
                // plugin failed here because ConditionalWeakTable is absent.
                type.GetFields(BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Static | BindingFlags.Instance);
                type.GetMethods(BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Static | BindingFlags.Instance);
            }
            var plugin = assembly.GetType("VamLibrary.SceneBrowser.SceneBrowserPlugin", true);
            System.Runtime.CompilerServices.RuntimeHelpers.RunClassConstructor(plugin.TypeHandle);
            // Exercise the state factory without instantiating a Unity component.
            var state = plugin.GetMethod("State", BindingFlags.Static | BindingFlags.NonPublic);
            if (state.Invoke(null, new object[] { null }) == null) throw new Exception("State factory failed");
            Console.WriteLine("PASS: plugin types, static initialization and browser state load in game Mono");
            var harmonyAssembly = Assembly.LoadFrom(Path.Combine(root, "BepInEx/core/0Harmony.dll"));
            var harmonyType = harmonyAssembly.GetType("HarmonyLib.Harmony", true);
            var harmony = Activator.CreateInstance(harmonyType, new object[] { "com.vamlibrary.runtime-test" });
            harmonyType.GetMethod("PatchAll", new[] { typeof(Assembly) }).Invoke(harmony, new object[] { assembly });
            int count = 0;
            foreach (object method in (System.Collections.IEnumerable)harmonyType.GetMethod("GetPatchedMethods").Invoke(harmony, null)) count++;
            if (count != 9) throw new Exception("Expected 9 runtime hooks, got " + count);
            harmonyType.GetMethod("UnpatchSelf").Invoke(harmony, null);
            Console.WriteLine("PASS: all 9 scene browser and scene launch Harmony hooks applied and removed in game Mono");
            foreach (string removed in new[] { "ZipLookupOptimization", "LoadingTelemetry", "LoadMetrics" })
                if (assembly.GetType("VamLibrary.SceneBrowser." + removed) != null) throw new Exception("Removed feature remains: " + removed);
            if (plugin.GetMethod("UpdateTelemetry", BindingFlags.NonPublic | BindingFlags.Instance) != null)
                throw new Exception("Loading overlay update remains");
            var decoder = assembly.GetType("VamLibrary.SceneBrowser.NativeDecompression", true);
            if (!(bool)decoder.GetMethod("Enable").Invoke(null, null)) throw new Exception("Native decompression hook failed to enable");
            var decoderHarmony = Activator.CreateInstance(harmonyType, new object[] { "com.vamlibrary.native-decompression" });
            int decoderHooks = 0;
            foreach (object method in (System.Collections.IEnumerable)harmonyType.GetMethod("GetPatchedMethods").Invoke(decoderHarmony, null)) decoderHooks++;
            if (decoderHooks != 1) throw new Exception("Expected 1 isolated decompression hook, got " + decoderHooks);
            decoder.GetMethod("Disable").Invoke(null, null);
            Console.WriteLine("PASS: isolated native decompression hook, no loading statistics or lookup optimization");
            var launchType = assembly.GetType("VamLibrary.SceneBrowser.SceneLaunchRequest", true);
            long now = (long)launchType.GetProperty("Now").GetValue(null, null);
            string launchText = "# VAM Library scene launch v1\nabc-1\n" + now + "\n作者.Scene.1:/Saves/scene/中文 场景.json\n";
            object request = launchType.GetMethod("Parse").Invoke(null, new object[] { launchText, "abc-1", now });
            if ((string)launchType.GetField("Scene").GetValue(request) != "作者.Scene.1:/Saves/scene/中文 场景.json") throw new Exception("Scene launch wire contract failed in game Mono");
            Console.WriteLine("PASS: scene launch Unicode request parsed in game Mono");
            var rules = assembly.GetType("VamLibrary.SceneBrowser.BrowserRules", true);
            var indexType = assembly.GetType("VamLibrary.SceneBrowser.ImportTimeIndex", true);
            var parsed = (System.Collections.IDictionary)indexType.GetMethod("Parse").Invoke(null, new object[] {
                "# VAM Library import times v1\n作者.Scene.1\t1790827200000\nNew.Scene.1\t1790899200000\n" });
            if ((long)parsed["作者.Scene.1"] != 1790827200000L) throw new Exception("Import timestamp changed in game Mono");
            var compare = rules.GetMethod("CompareImported");
            if ((int)compare.Invoke(null, new object[] { 1790899200000L, 1790827200000L, "Z", "z", "1", "A", "a", "2" }) >= 0)
                throw new Exception("Newest software imports did not sort first");
            if ((int)compare.Invoke(null, new object[] { null, 1790827200000L, "A", "a", "1", "Z", "z", "2" }) <= 0)
                throw new Exception("Unindexed scenes did not sort last");
            string fixture = Path.Combine(Path.GetTempPath(), "MonoImportTimes-" + Guid.NewGuid().ToString("N") + ".tsv");
            try
            {
                File.WriteAllText(fixture, "# VAM Library import times v1\n作者.Scene.1\t1790827200000\n");
                var index = Activator.CreateInstance(indexType);
                indexType.GetMethod("Refresh").Invoke(index, new object[] { fixture, true });
                if ((long)indexType.GetMethod("Find").Invoke(index, new object[] { "作者.Scene.1" }) != 1790827200000L)
                    throw new Exception("Game Mono could not read the UTF-8 import-time snapshot");
            }
            finally { if (File.Exists(fixture)) File.Delete(fixture); }
            Console.WriteLine("PASS: software import-time parsing and sorting in game Mono");
            var folders = (System.Collections.IList)rules.GetMethod("ListFolders").Invoke(null, new object[] { Path.Combine(root, "AddonPackages"), "" });
            if (folders.Count == 0) throw new Exception("Expected folders in the regression game library");
            var createItem = plugin.GetMethod("CreateNavigationItem", BindingFlags.Static | BindingFlags.NonPublic);
            foreach (object folder in folders)
            {
                string relative = (string)folder.GetType().GetField("RelativePath").GetValue(folder);
                object item = createItem.Invoke(null, new object[] { Path.Combine(root, "AddonPackages/" + relative) });
                var type = item.GetType();
                if (!(bool)type.GetProperty("isDirectory").GetValue(item, null)) throw new Exception("Not a native grid directory item");
                if (type.GetProperty("DirectoryEntry").GetValue(item, null) != null || type.GetProperty("FileEntry").GetValue(item, null) != null)
                    throw new Exception("Navigation would be hidden by native search filtering");
            }
            Console.WriteLine("PASS: " + folders.Count + " real AddonPackages folders become native grid navigation entries without a sidebar");
            return 0;
        }
        catch (Exception e)
        {
            Console.WriteLine(e);
            var load = e as ReflectionTypeLoadException;
            if (load != null) foreach (var inner in load.LoaderExceptions) Console.WriteLine(inner);
            return 1;
        }
    }
}

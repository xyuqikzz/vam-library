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
            if (count != 8) throw new Exception("Expected 8 runtime hooks, got " + count);
            harmonyType.GetMethod("UnpatchSelf").Invoke(harmony, null);
            Console.WriteLine("PASS: all 8 scene browser Harmony hooks applied and removed in game Mono");
            var rules = assembly.GetType("VamLibrary.SceneBrowser.BrowserRules", true);
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

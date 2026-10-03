using System;
using System.IO;
using System.Collections.Generic;
using System.Diagnostics;
using System.Drawing;
using System.Drawing.Imaging;
using System.Reflection;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Threading;
using HarmonyLib;
using ICSharpCode.SharpZipLib.Checksums;
using ICSharpCode.SharpZipLib.Zip;
using ICSharpCode.SharpZipLib.Zip.Compression;
using ICSharpCode.SharpZipLib.Zip.Compression.Streams;
using VamLibrary.SceneBrowser;

class NativeDecompressionTests
{
    static int checks;
    static void Check(bool condition, string message) { Interlocked.Increment(ref checks); if (!condition) throw new Exception(message); }
    static void Enable() { Check(NativeDecompression.Enable(), "Cannot enable: " + NativeDecompression.Failure); }
    static string Hash(byte[] bytes) { using (var hash = SHA256.Create()) return Convert.ToBase64String(hash.ComputeHash(bytes)); }
    static bool Accelerated(Stream stream) { return stream.GetType().Name == "NativeInflaterStream"; }
    static byte[] Read(Stream stream, int block)
    {
        using (var data = new MemoryStream())
        {
            byte[] buffer = new byte[block]; int count;
            while ((count = stream.Read(buffer, 0, buffer.Length)) > 0) data.Write(buffer, 0, count);
            Check(stream.Read(buffer, 0, buffer.Length) == 0, "EOF changed");
            return data.ToArray();
        }
    }
    static void Pack(string path, byte[][] data, bool stored, string password)
    {
        using (var output = new ZipOutputStream(File.Create(path)))
        {
            output.Password = password;
            for (int i = 0; i < data.Length; i++)
            {
                var entry = new ZipEntry("中文/entry" + i + ".bin"); entry.IsUnicodeText = true; entry.Size = data[i].Length;
                if (stored) { var crc = new Crc32(); crc.Update(data[i]); entry.CompressionMethod = CompressionMethod.Stored; entry.Crc = crc.Value; }
                output.PutNextEntry(entry); output.Write(data[i], 0, data[i].Length); output.CloseEntry();
            }
            output.Finish();
        }
    }
    static byte[] Decode(string path, int index, bool native, int block)
    {
        if (native) Enable(); else NativeDecompression.Disable();
        using (var zip = new ZipFile(path)) using (var stream = zip.GetInputStream(index)) return Read(stream, block);
    }
    static void OtherPatch() { }
    sealed class FailOnceStream : MemoryStream
    {
        bool failed;
        public FailOnceStream(byte[] bytes) : base(bytes) { }
        public override int Read(byte[] buffer, int offset, int count)
        {
            if (!failed && Position >= 16384) { failed = true; throw new IOException("Injected decoder input failure"); }
            return base.Read(buffer, offset, count);
        }
    }
    static void FaultTests(byte[] data)
    {
        var packed = new MemoryStream();
        using (var stream = new DeflaterOutputStream(packed, new Deflater(6, true))) { stream.IsStreamOwner = false; stream.Write(data, 0, data.Length); stream.Finish(); }
        var type = typeof(NativeDecompression).GetNestedType("NativeInflaterStream", BindingFlags.NonPublic);
        var crc = new Crc32(); crc.Update(data);
        using (var source = new FailOnceStream(packed.ToArray()))
        using (var stream = (Stream)Activator.CreateInstance(type, new object[] { source, new Inflater(true), (long)data.Length, crc.Value }))
            Check(Hash(Read(stream, 997)) == Hash(data), "Mid-read fallback changed bytes or lost position");
        Check(!NativeDecompression.Enabled, "Decoder failure did not disable new native streams");
        foreach (long size in new[] { (long)data.Length - 1, (long)data.Length + 1 })
        {
            Enable();
            using (var source = new MemoryStream(packed.ToArray()))
            using (var stream = (Stream)Activator.CreateInstance(type, new object[] { source, new Inflater(true), size, crc.Value }))
                Check(Hash(Read(stream, 5000)) == Hash(data), "Size mismatch did not replay original decoder");
            Check(!NativeDecompression.Enabled, "Bad size was silently accepted");
        }
        Enable();
        using (var source = new MemoryStream(packed.ToArray()))
        using (var stream = (Stream)Activator.CreateInstance(type, new object[] { source, new Inflater(true), (long)data.Length, crc.Value ^ 1 }))
            Check(Hash(Read(stream, 5000)) == Hash(data), "CRC disagreement changed original behavior");
        Check(!NativeDecompression.Enabled, "CRC mismatch was silently accepted");
        foreach (byte[] corrupt in new[] { new byte[] { 7, 255, 255 }, Cut(packed.ToArray()) })
        {
            Enable(); bool threw = false;
            try
            {
                using (var source = new MemoryStream(corrupt))
                using (var stream = (Stream)Activator.CreateInstance(type, new object[] { source, new Inflater(true), (long)data.Length, crc.Value })) Read(stream, 4096);
            }
            catch (ICSharpCode.SharpZipLib.SharpZipBaseException) { threw = true; }
            Check(threw, "Corrupt Deflate stream did not preserve original decoder exception");
        }
    }
    static void ChecksumTests()
    {
        var type = typeof(NativeDecompression).GetNestedType("BlockCrc32", BindingFlags.NonPublic);
        var update = type.GetMethod("Update"); var value = type.GetProperty("Value");
        var known = Activator.CreateInstance(type, true); var text = System.Text.Encoding.ASCII.GetBytes("123456789");
        update.Invoke(known, new object[] { text, 0, text.Length });
        Check((long)value.GetValue(known, null) == 0xcbf43926L, "IEEE CRC-32 check vector failed");
        var random = new Random(91);
        foreach (int length in new[] { 0, 1, 7, 8, 9, 15, 16, 17, 256, 4097, 65539 })
        {
            var bytes = new byte[length + 11]; random.NextBytes(bytes); var reference = new Crc32(); reference.Update(bytes, 5, length);
            foreach (int block in new[] { 3, 8, 999 })
            {
                var crc = Activator.CreateInstance(type, true); int offset = 5;
                while (offset < length + 5) { int count = Math.Min(block, length + 5 - offset); update.Invoke(crc, new object[] { bytes, offset, count }); offset += count; }
                Check((long)value.GetValue(crc, null) == reference.Value, "CRC offset/chunk boundary mismatch");
            }
        }
    }
    static byte[] Cut(byte[] bytes) { var result = new byte[bytes.Length / 2]; Array.Copy(bytes, result, result.Length); return result; }
    static void Contracts(string root)
    {
        var random = new Random(71); var randomData = new byte[524288]; random.NextBytes(randomData);
        for (int i = 0; i < randomData.Length; i++) if (i % 9 != 0) randomData[i] = (byte)(i % 99);
        var repeated = new byte[2 * 1024 * 1024]; for (int i = 0; i < repeated.Length; i++) repeated[i] = (byte)(i % 117);
        var boundary = new byte[65536]; random.NextBytes(boundary);
        byte[][] data = { new byte[0], new byte[] { 1, 2, 3 }, boundary, randomData, repeated };
        string path = Path.Combine(root, "fixture.var"); Pack(path, data, false, null);
        for (int i = 0; i < data.Length; i++)
        {
            Check(Hash(Decode(path, i, false, 8192)) == Hash(data[i]), "Original fixture mismatch");
            foreach (int block in new[] { 1, 997, 32768, 131072 })
            {
                Enable(); using (var zip = new ZipFile(path)) using (var stream = zip.GetInputStream(zip[i]))
                {
                    Check(Accelerated(stream) == (i == 3 || i == 4), "Compressed/incompressible entry eligibility mismatch");
                    Check(stream.Read(new byte[1], 0, 0) == 0, "Zero read consumed input");
                    Check(Hash(Read(stream, block)) == Hash(data[i]), "Decoded bytes differ for chunk " + block);
                    Check(NativeDecompression.Enabled, "Unexpected fallback: " + NativeDecompression.Failure);
                }
            }
        }
        Enable();
        using (var zip = new ZipFile(path))
        {
            using (var stream = zip.GetInputStream(3))
            {
                bool rejected = false;
                try { stream.Read(new byte[10], 9, 2); } catch (ArgumentException) { rejected = true; }
                Check(rejected, "Invalid range accepted");
                Check(Hash(Read(stream, 4096)) == Hash(randomData), "Invalid read changed stream position");
                stream.Close(); rejected = false;
                try { stream.ReadByte(); } catch (ObjectDisposedException) { rejected = true; }
                Check(rejected, "Closed native stream remained readable");
            }
            using (var stream = zip.GetInputStream(3))
            {
                Check(stream.ReadByte() == randomData[0], "ReadByte bypassed new decoder");
                NativeDecompression.Disable();
                byte[] rest = Read(stream, 8000); Check(rest.Length == randomData.Length - 1, "Disable broke an active stream");
            }
            Enable();
            using (var stream = zip.GetInputStream(3)) { var buffer = new byte[4096]; stream.Read(buffer, 0, 4096); }
            using (var stream = zip.GetInputStream(3)) Check(Hash(Read(stream, 32768)) == Hash(randomData), "Partial close damaged archive");
            Exception failure = null; var threads = new List<Thread>();
            for (int t = 0; t < 4; t++)
            {
                var thread = new Thread(delegate() { try { for (int j = 0; j < 3; j++) using (var stream = zip.GetInputStream(3)) if (Hash(Read(stream, 997)) != Hash(randomData)) throw new Exception("Concurrent output mismatch"); } catch (Exception e) { failure = e; } });
                threads.Add(thread); thread.Start();
            }
            foreach (var thread in threads) thread.Join(); Check(failure == null, "Concurrent reads failed: " + failure);
        }
        foreach (string variant in new[] { "nonvar.zip", "stored.var", "encrypted.var" })
        {
            string alternate = Path.Combine(root, variant); Pack(alternate, new[] { randomData }, variant == "stored.var", variant == "encrypted.var" ? "test" : null);
            Enable(); using (var zip = new ZipFile(alternate)) { zip.Password = "test"; using (var stream = zip.GetInputStream(0)) { Check(!Accelerated(stream), "Unsafe entry accelerated"); Check(Hash(Read(stream, 8192)) == Hash(randomData), "Fallback entry mismatch"); } }
        }
        NativeDecompression.Disable();
        var target = AccessTools.Method(typeof(ZipFile), "GetInputStream", new[] { typeof(long) });
        var other = new Harmony("test.other-decompressor");
        try
        {
            other.Patch(target, prefix: new HarmonyMethod(typeof(NativeDecompressionTests), "OtherPatch"));
            Check(!NativeDecompression.Enable(), "Existing decoder patch conflict was ignored");
            other.UnpatchSelf(); Enable();
            other.Patch(target, postfix: new HarmonyMethod(typeof(NativeDecompressionTests), "OtherPatch"));
            using (var zip = new ZipFile(path)) using (var stream = zip.GetInputStream(3))
                Check(!Accelerated(stream) && Hash(Read(stream, 8192)) == Hash(randomData), "Later patch did not preserve original decoder");
            Check(!NativeDecompression.Enabled, "Later conflict did not disable acceleration");
            var find = AccessTools.Method(typeof(ZipFile), "FindEntry", new[] { typeof(string), typeof(bool) });
            Check(Harmony.GetPatchInfo(find) == null || Harmony.GetPatchInfo(find).Owners.Count == 0, "Lookup optimization remains installed");
        }
        finally { other.UnpatchSelf(); NativeDecompression.Disable(); }
        FaultTests(randomData);
        ChecksumTests();
        Console.WriteLine("PASS: " + checks + " decompression compatibility checks in game Mono");
        Benchmark(path, new[] { 3, 4 }, false);
    }
    static byte[] Pixels(Stream stream)
    {
        using (var image = new Bitmap(stream))
        {
            image.RotateFlip(RotateFlipType.RotateNoneFlipY);
            var bits = image.LockBits(new Rectangle(0, 0, image.Width, image.Height), ImageLockMode.ReadOnly, PixelFormat.Format32bppArgb);
            try
            {
                byte[] result = new byte[checked(image.Width * image.Height * 4)];
                for (int row = 0; row < image.Height; row++) Marshal.Copy(new IntPtr(bits.Scan0.ToInt64() + row * (long)bits.Stride), result, row * image.Width * 4, image.Width * 4);
                return result;
            }
            finally { image.UnlockBits(bits); }
        }
    }
    static void Benchmark(string path, int[] entries, bool images)
    {
        double[][] times = { new double[5], new double[5] }; string reference = null; long size = 0; int accelerated = 0;
        // First pair warms both code paths. Remaining pairs alternate order.
        for (int round = -1; round < 5; round++) for (int pass = 0; pass < 2; pass++)
        {
            int mode = (round + 1 + pass) % 2;
            if (mode == 1) Enable(); else NativeDecompression.Disable();
            var output = new List<byte[]>(); int used = 0; var watch = Stopwatch.StartNew();
            using (var zip = new ZipFile(path)) foreach (int index in entries)
                using (var stream = zip.GetInputStream(index)) { if (Accelerated(stream)) used++; output.Add(images ? Pixels(stream) : Read(stream, 65536)); }
            watch.Stop();
            string digest = ""; size = 0;
            foreach (var data in output) { digest += Hash(data); size += data.Length; }
            if (reference == null) reference = digest;
            Check(reference == digest, "Benchmark output/pixels differ");
            Check(mode == 0 || NativeDecompression.Enabled, "Benchmark silently fell back: " + NativeDecompression.Failure);
            if (mode == 1) accelerated = used;
            if (round >= 0) times[mode][round] = watch.Elapsed.TotalMilliseconds;
        }
        Array.Sort(times[0]); Array.Sort(times[1]);
        Console.WriteLine("BENCH " + Path.GetFileName(path) + (images ? " unzip+GDI_decode+flip+pixels" : " unzip") + " entries=" + entries.Length + " accelerated=" + accelerated + " bytes=" + size + " original=" + times[0][2].ToString("F2") + "ms native=" + times[1][2].ToString("F2") + "ms");
    }
    static int Main(string[] args)
    {
        Assembly.LoadFrom(args[1]);
        return Run(args);
    }
    [System.Runtime.CompilerServices.MethodImpl(System.Runtime.CompilerServices.MethodImplOptions.NoInlining)]
    static int Run(string[] args)
    {
        ZipConstants.DefaultCodePage = 65001;
        string temp = Path.Combine(Path.GetTempPath(), "VamInflateCases-" + Guid.NewGuid().ToString("N")); Directory.CreateDirectory(temp);
        try
        {
            Contracts(temp);
            foreach (string relative in new[] { "纹理/DJ.TanLines.2.var", "依赖旧版本/DJ.TanLines.1.var", "其他/Amaimon.Bhaviour.2.var" })
            {
                string path = Path.Combine(Path.Combine(args[0], "AddonPackages"), relative);
                if (!File.Exists(path)) { Console.WriteLine("SKIP absent real sample " + relative); continue; }
                var selected = new List<int>(); var images = new List<int>();
                using (var zip = new ZipFile(path)) for (int i = 0; i < zip.Count; i++)
                {
                    var entry = zip[i];
                    if (!entry.IsCrypted && entry.CompressionMethod == CompressionMethod.Deflated && entry.Size >= 65536 && entry.Size <= 32 * 1024 * 1024)
                    {
                        if (selected.Count < 3) selected.Add(i);
                        string extension = Path.GetExtension(entry.Name).ToLowerInvariant();
                        if (images.Count < 1 && (extension == ".jpg" || extension == ".png")) images.Add(i);
                    }
                }
                if (selected.Count > 0) Benchmark(path, selected.ToArray(), false);
                if (images.Count > 0) Benchmark(path, images.ToArray(), true);
            }
            Console.WriteLine("PASS: " + checks + " total checks, byte and decoded-pixel identity verified"); return 0;
        }
        catch (Exception error) { Console.WriteLine(error); return 1; }
        finally { NativeDecompression.Disable(); foreach (string file in Directory.GetFiles(temp)) File.Delete(file); Directory.Delete(temp); }
    }
}

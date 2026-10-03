using System;
using System.Collections.Generic;
using System.IO;
using System.IO.Compression;
using System.Reflection;
using System.Reflection.Emit;
using HarmonyLib;
using ICSharpCode.SharpZipLib.Zip;
using ICSharpCode.SharpZipLib.Zip.Compression;
using ICSharpCode.SharpZipLib.Zip.Compression.Streams;

namespace VamLibrary.SceneBrowser
{
    // Keep ZipFile's own lookup, local-header checks and bounded input stream.
    // Only substitute the Deflate decoder, using the zlib shipped with game Mono.
    public static class NativeDecompression
    {
        private const string Owner = "com.vamlibrary.native-decompression";
        private static Harmony harmony;
        private static MethodInfo target;
        private static volatile bool enabled;
        private static volatile string failure;
        public static bool Enabled { get { return enabled; } }
        public static string Failure { get { return failure; } }

        public static bool Enable()
        {
            if (enabled) return true;
            Disable(); failure = null;
            try
            {
                // No installing/replacing a native library: test the game's runtime first.
                using (var input = new MemoryStream(new byte[] { 203, 72, 205, 201, 201, 7, 0 }))
                using (var stream = new DeflateStream(input, CompressionMode.Decompress))
                {
                    foreach (byte value in new byte[] { 104, 101, 108, 108, 111 })
                        if (stream.ReadByte() != value) throw new InvalidDataException("Native Deflate self-test failed");
                    if (stream.ReadByte() != -1) throw new InvalidDataException("Native Deflate end-of-stream mismatch");
                }
                target = AccessTools.Method(typeof(ZipFile), "GetInputStream", new[] { typeof(long) });
                if (target == null || HasOtherPatches(false)) throw new InvalidOperationException("ZIP decompression API unavailable or already patched");
                harmony = new Harmony(Owner);
                harmony.Patch(target, transpiler: new HarmonyMethod(typeof(NativeDecompression), "ReplaceInflater"));
                enabled = true;
                return true;
            }
            catch (Exception error) { Disable(); failure = error.Message; return false; }
        }
        public static void Disable()
        {
            enabled = false;
            if (harmony != null) { harmony.UnpatchSelf(); harmony = null; }
        }
        private static bool HasOtherPatches(bool installed)
        {
            var info = Harmony.GetPatchInfo(target);
            if (info == null) return installed;
            return info.Prefixes.Count != 0 || info.Postfixes.Count != 0 || info.Finalizers.Count != 0 ||
                info.Transpilers.Count != (installed ? 1 : 0) || (installed && info.Transpilers[0].owner != Owner);
        }
        internal static void Fail(Exception error) { enabled = false; failure = error.Message; }

        private static IEnumerable<CodeInstruction> ReplaceInflater(IEnumerable<CodeInstruction> instructions)
        {
            var constructor = AccessTools.Constructor(typeof(InflaterInputStream), new[] { typeof(Stream), typeof(Inflater) });
            var factory = AccessTools.Method(typeof(NativeDecompression), "CreateInflater");
            var result = new List<CodeInstruction>(); int replacements = 0;
            foreach (var instruction in instructions)
            {
                if (instruction.opcode == OpCodes.Newobj && Equals(instruction.operand, constructor))
                {
                    // Branches/exception boundaries must enter before the extra arguments.
                    var archive = new CodeInstruction(OpCodes.Ldarg_0);
                    archive.labels.AddRange(instruction.labels); archive.blocks.AddRange(instruction.blocks);
                    instruction.labels.Clear(); instruction.blocks.Clear();
                    result.Add(archive); result.Add(new CodeInstruction(OpCodes.Ldarg_1));
                    instruction.opcode = OpCodes.Call; instruction.operand = factory; replacements++;
                }
                result.Add(instruction);
            }
            if (replacements != 1) throw new MissingMethodException("Unsupported ZIP inflater construction");
            return result;
        }
        private static Stream CreateInflater(Stream input, Inflater inflater, ZipFile archive, long index)
        {
            try
            {
                if (enabled && archive.Name != null && archive.Name.EndsWith(".var", StringComparison.OrdinalIgnoreCase) && !archive.IsUpdating)
                {
                    if (HasOtherPatches(true)) throw new InvalidOperationException("Another plugin changed ZIP decompression; using original decoder");
                    var entry = archive[checked((int)index)];
                    // Deflate entries with no size reduction commonly contain stored
                    // blocks: copying them is already cheap, and zlib+CRC adds overhead.
                    if (!entry.IsCrypted && entry.Size >= 65536 && entry.CompressedSize > 0 && entry.CompressedSize < entry.Size &&
                        entry.Crc >= 0 && input.CanSeek && input.Position == 0)
                        return new NativeInflaterStream(input, inflater, entry.Size, entry.Crc);
                }
            }
            catch (Exception error) { Fail(error); }
            return new InflaterInputStream(input, inflater);
        }

        private sealed class NativeInflaterStream : InflaterInputStream
        {
            private readonly Stream source;
            private readonly ReadObserver input;
            private readonly long expectedSize, expectedCrc;
            private readonly BlockCrc32 crc = new BlockCrc32();
            private DeflateStream native;
            private long delivered;
            private bool closed, verified;
            private readonly byte[] endProbe = new byte[1];

            public NativeInflaterStream(Stream source, Inflater inflater, long size, long crcValue) : base(source, inflater)
            {
                this.source = source; expectedSize = size; expectedCrc = crcValue;
                input = new ReadObserver(source);
                native = new DeflateStream(input, CompressionMode.Decompress, true);
            }
            public override long Length { get { return native == null ? base.Length : input.LastRead; } }
            public override int Read(byte[] buffer, int offset, int count)
            {
                if (closed) throw new ObjectDisposedException("InflaterInputStream");
                if (buffer == null) throw new ArgumentNullException("buffer");
                if (offset < 0) throw new ArgumentOutOfRangeException("offset");
                if (count < 0) throw new ArgumentOutOfRangeException("count");
                if (offset > buffer.Length - count) throw new ArgumentException("Invalid buffer range");
                if (native == null) return base.Read(buffer, offset, count);
                if (count == 0 || verified) return 0;
                int total = 0;
                try
                {
                    while (total < count)
                    {
                        int read = native.Read(buffer, offset + total, count - total);
                        if (read == 0) break;
                        crc.Update(buffer, offset + total, read); total += read;
                        if (delivered + total >= expectedSize) break;
                    }
                    if (delivered + total > expectedSize) throw new InvalidDataException("Deflate output exceeds ZIP size");
                    if (delivered + total == expectedSize)
                    {
                        if (native.Read(endProbe, 0, 1) != 0 || crc.Value != expectedCrc)
                            throw new InvalidDataException("Deflate output does not match ZIP size/CRC");
                        verified = true;
                    }
                    else if (total < count) throw new InvalidDataException("Deflate output ended early");
                }
                catch (Exception error)
                {
                    if (error is OutOfMemoryException || error is System.Threading.ThreadAbortException) throw;
                    Fail(error);
                    // The current read has not been returned. Rewind/replay the ORIGINAL
                    // inflater up to the last delivered byte, then overwrite this read.
                    // Never turn a decoder error into a successful short/empty read.
                    FallBack();
                    return base.Read(buffer, offset, count);
                }
                delivered += total;
                return total;
            }
            private void FallBack()
            {
                var decoder = native; native = null;
                try { decoder.Dispose(); }
                finally { source.Seek(0, SeekOrigin.Begin); }
                byte[] discard = new byte[32768]; long remaining = delivered;
                while (remaining > 0)
                {
                    int read = base.Read(discard, 0, (int)Math.Min(remaining, discard.Length));
                    if (read == 0) throw new EndOfStreamException("Original ZIP stream ended while restoring position");
                    remaining -= read;
                }
            }
            public override void Close()
            {
                if (closed) return;
                closed = true;
                try { if (native != null) native.Dispose(); }
                finally { native = null; base.Close(); }
            }
        }
        // Preserve the inflater stream's Length convention (last input block size).
        // All reads still use ZipFile.PartialInputStream's archive lock and bounds.
        private sealed class ReadObserver : Stream
        {
            private readonly Stream source;
            public int LastRead;
            public ReadObserver(Stream source) { this.source = source; }
            public override int Read(byte[] buffer, int offset, int count) { LastRead = source.Read(buffer, offset, count); return LastRead; }
            public override bool CanRead { get { return source.CanRead; } }
            public override bool CanSeek { get { return false; } }
            public override bool CanWrite { get { return false; } }
            public override long Length { get { return source.Length; } }
            public override long Position { get { return source.Position; } set { throw new NotSupportedException(); } }
            public override void Flush() { source.Flush(); }
            public override long Seek(long offset, SeekOrigin origin) { throw new NotSupportedException(); }
            public override void SetLength(long value) { throw new NotSupportedException(); }
            public override void Write(byte[] buffer, int offset, int count) { throw new NotSupportedException(); }
        }
        // IEEE CRC-32, eight bytes per iteration. Byte-at-a-time managed CRC
        // can cost more than native inflate on already-compressed images.
        private sealed class BlockCrc32
        {
            private static readonly uint[][] tables = MakeTables();
            private uint value = 0xffffffff;
            public long Value { get { return value ^ 0xffffffff; } }
            private static uint[][] MakeTables()
            {
                var result = new uint[8][];
                for (int i = 0; i < 8; i++) result[i] = new uint[256];
                for (uint i = 0; i < 256; i++)
                {
                    uint item = i;
                    for (int bit = 0; bit < 8; bit++) item = (item >> 1) ^ ((item & 1) != 0 ? 0xedb88320 : 0);
                    result[0][i] = item;
                }
                for (int slice = 1; slice < 8; slice++) for (int i = 0; i < 256; i++)
                {
                    uint item = result[slice - 1][i];
                    result[slice][i] = (item >> 8) ^ result[0][item & 255];
                }
                return result;
            }
            public void Update(byte[] bytes, int offset, int count)
            {
                int end = offset + count;
                while (offset <= end - 8)
                {
                    uint first = value ^ (uint)(bytes[offset] | bytes[offset + 1] << 8 | bytes[offset + 2] << 16 | bytes[offset + 3] << 24);
                    value = tables[7][first & 255] ^ tables[6][(first >> 8) & 255] ^ tables[5][(first >> 16) & 255] ^ tables[4][first >> 24] ^
                        tables[3][bytes[offset + 4]] ^ tables[2][bytes[offset + 5]] ^ tables[1][bytes[offset + 6]] ^ tables[0][bytes[offset + 7]];
                    offset += 8;
                }
                while (offset < end) value = (value >> 8) ^ tables[0][(value ^ bytes[offset++]) & 255];
            }
        }
    }
}

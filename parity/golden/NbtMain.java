/*
 * CeriumMC NBT and SNBT recorder. Copyright StepByte Studios, GPL-2.0-only.
 *
 * Records what vanilla's net.minecraft.nbt does with random and malformed inputs: binary
 * reading and writing in the file (DataInputStream/DataOutputStream) and packet (Netty
 * ByteBuf streams) forms, quota and depth accounting, toString, the structure SNBT printer,
 * hashCode, sizeInBytes, copy order, the streaming field visitors, and SNBT parsing with its
 * exact errors. It runs vanilla's code, so its output stays in the CI cache.
 *
 * Usage: java -cp <classes>:<server classpath> golden.NbtMain <output-dir>
 */
package golden;

import com.mojang.brigadier.exceptions.CommandSyntaxException;
import io.netty.buffer.ByteBuf;
import io.netty.buffer.ByteBufInputStream;
import io.netty.buffer.ByteBufOutputStream;
import io.netty.buffer.Unpooled;
import java.io.BufferedWriter;
import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.DataInput;
import java.io.DataInputStream;
import java.io.DataOutputStream;
import java.io.IOException;
import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HexFormat;
import java.util.List;
import java.util.SplittableRandom;
import net.minecraft.CrashReport;
import net.minecraft.CrashReportCategory;
import net.minecraft.ReportedException;
import net.minecraft.nbt.ByteArrayTag;
import net.minecraft.nbt.ByteTag;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.DoubleTag;
import net.minecraft.nbt.EndTag;
import net.minecraft.nbt.FloatTag;
import net.minecraft.nbt.IntArrayTag;
import net.minecraft.nbt.IntTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.LongArrayTag;
import net.minecraft.nbt.LongTag;
import net.minecraft.nbt.NbtAccounter;
import net.minecraft.nbt.NbtIo;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.ShortTag;
import net.minecraft.nbt.SnbtPrinterTagVisitor;
import net.minecraft.nbt.StringTag;
import net.minecraft.nbt.Tag;
import net.minecraft.nbt.TagParser;
import net.minecraft.nbt.TagTypes;
import net.minecraft.nbt.visitors.CollectFields;
import net.minecraft.nbt.visitors.FieldSelector;
import net.minecraft.nbt.visitors.SkipFields;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.contents.TranslatableContents;

public final class NbtMain {
    private static final HexFormat HEX = HexFormat.of();
    private final SplittableRandom rnd = new SplittableRandom(0x7A7A_0004L);
    private final BufferedWriter out;
    private long cases;

    private NbtMain(BufferedWriter out) {
        this.out = out;
    }

    public static void main(String[] args) throws Exception {
        // Vanilla's 512-level depth check, not the JVM's stack size, must decide deep inputs,
        // so record on a thread with a large stack.
        Throwable[] failure = new Throwable[1];
        Thread worker = new Thread(null, () -> {
            try {
                record(args);
            } catch (Throwable t) {
                failure[0] = t;
            }
        }, "nbt-golden", 512L << 20);
        worker.start();
        worker.join();
        if (failure[0] != null) {
            throw new IllegalStateException("recording failed", failure[0]);
        }
    }

    private static void record(String[] args) throws Exception {
        // Crash reports (built for every ReportedNbtException) look the version up.
        net.minecraft.SharedConstants.tryDetectVersion();
        Path dir = Path.of(args[0]);
        Files.createDirectories(dir);
        try (BufferedWriter out = Files.newBufferedWriter(dir.resolve("nbt.txt"), StandardCharsets.UTF_8)) {
            NbtMain main = new NbtMain(out);
            // An optional second argument limits the phases, for local runs.
            String phases = args.length > 1 ? args[1] : "tags,malformed,visitors,snbt";
            long t0 = System.nanoTime();
            if (phases.contains("tags")) {
                main.tags();
            }
            long t1 = System.nanoTime();
            if (phases.contains("malformed")) {
                main.malformed();
            }
            long t2 = System.nanoTime();
            if (phases.contains("visitors")) {
                main.visitors();
            }
            long t3 = System.nanoTime();
            if (phases.contains("snbt")) {
                main.snbt();
            }
            long t4 = System.nanoTime();
            System.out.printf("seconds: tags %.1f, malformed %.1f, visitors %.1f, snbt %.1f%n",
                (t1 - t0) / 1e9, (t2 - t1) / 1e9, (t3 - t2) / 1e9, (t4 - t3) / 1e9);
            System.out.println("nbt.txt: " + main.cases + " cases");
        }
    }

    // ---------------------------------------------------------------- tokens

    static String u(String s) {
        StringBuilder b = new StringBuilder("u:");
        for (int i = 0; i < s.length(); i++) {
            b.append(HEX.toHexDigits(s.charAt(i)));
        }
        return b.toString();
    }

    static String x(byte[] bytes) {
        return "x:" + HEX.formatHex(bytes);
    }

    private void line(String op, List<String> args, List<String> results) throws IOException {
        StringBuilder b = new StringBuilder(op);
        for (String a : args) {
            b.append(' ').append(a);
        }
        b.append(" =");
        for (String r : results) {
            b.append(' ').append(r);
        }
        b.append('\n');
        out.write(b.toString());
        cases++;
    }

    // ---------------------------------------------------------------- random tags

    private static final String[] KEYS = {"pos", "state", "nbt", "DataVersion", "id", "Count", "data",
        "size", "palette", "palettes", "entities", "blockPos", "author", "true", "FALSE", "a b", "",
        "1st", "_x", ".dot", "plus+", "Aa", "BB", "AaAa", "AaBB", "BBAa", "BBBB", "AaAaAa", "AaAaBB",
        "AaBBAa", "AaBBBB", "BBAaAa", "BBAaBB", "BBBBAa", "BBBBBB", "q\"uote", "s'ingle", "back\\slash",
        "\u00e9t\u00e9", "\uD800lone", "tab\there", "ctl\u0001x"};

    private String randomString() {
        int n = rnd.nextInt(rnd.nextInt(10) == 0 ? 40 : 8);
        StringBuilder b = new StringBuilder();
        for (int i = 0; i < n; i++) {
            b.append(switch (rnd.nextInt(8)) {
                case 0 -> (char) rnd.nextInt(0x20);
                case 1 -> "\"'\\".charAt(rnd.nextInt(3));
                case 2 -> (char) rnd.nextInt(0x100);
                case 3 -> (char) (0xD800 + rnd.nextInt(0x800));
                case 4 -> (char) rnd.nextInt(0x10000);
                default -> (char) ('a' + rnd.nextInt(26));
            });
        }
        return b.toString();
    }

    private String randomKey() {
        return rnd.nextInt(3) == 0 ? randomString() : KEYS[rnd.nextInt(KEYS.length)];
    }

    private float randomFloat() {
        return switch (rnd.nextInt(5)) {
            case 0 -> Float.intBitsToFloat(rnd.nextInt());
            case 1 -> Float.intBitsToFloat(0x7FC00000 | rnd.nextInt(0x400000)); // NaN payloads
            case 2 -> (float) rnd.nextInt(1000) / 8;
            default -> (float) (rnd.nextDouble() * Math.pow(10, rnd.nextInt(20) - 10));
        };
    }

    private double randomDouble() {
        return switch (rnd.nextInt(5)) {
            case 0 -> Double.longBitsToDouble(rnd.nextLong());
            case 1 -> Double.longBitsToDouble(0x7FF8000000000000L | rnd.nextLong(0x8000000000000L));
            case 2 -> (double) rnd.nextInt(100000) / 16;
            default -> rnd.nextDouble() * Math.pow(10, rnd.nextInt(40) - 20);
        };
    }

    private Tag randomTag(int depth) {
        int kind = rnd.nextInt(depth > 4 ? 10 : 13);
        return switch (kind) {
            case 0 -> ByteTag.valueOf((byte) rnd.nextInt());
            case 1 -> ShortTag.valueOf((short) rnd.nextInt());
            case 2 -> IntTag.valueOf(rnd.nextBoolean() ? rnd.nextInt() : rnd.nextInt(-200, 2000));
            case 3 -> LongTag.valueOf(rnd.nextLong());
            case 4 -> FloatTag.valueOf(randomFloat());
            case 5 -> DoubleTag.valueOf(randomDouble());
            case 6 -> StringTag.valueOf(randomString());
            case 7 -> {
                byte[] data = new byte[rnd.nextInt(6)];
                rnd.nextBytes(data);
                yield new ByteArrayTag(data);
            }
            case 8 -> new IntArrayTag(rnd.ints(rnd.nextInt(5)).toArray());
            case 9 -> new LongArrayTag(rnd.longs(rnd.nextInt(4)).toArray());
            case 10, 11 -> {
                CompoundTag compound = new CompoundTag();
                int n = rnd.nextInt(rnd.nextInt(8) == 0 ? 30 : 6);
                for (int i = 0; i < n; i++) {
                    compound.put(randomKey(), randomTag(depth + 1));
                    if (rnd.nextInt(10) == 0) {
                        compound.remove(randomKey());
                    }
                }
                yield compound;
            }
            default -> {
                ListTag list = new ListTag();
                int n = rnd.nextInt(6);
                boolean mixed = rnd.nextInt(3) == 0;
                Tag first = randomTag(depth + 1);
                for (int i = 0; i < n; i++) {
                    list.add(mixed ? randomTag(depth + 1) : sameType(first, depth + 1));
                }
                yield list;
            }
        };
    }

    /** A random tag of the same type as {@code tag} (lists of one type). */
    private Tag sameType(Tag tag, int depth) {
        for (int tries = 0; tries < 50; tries++) {
            Tag candidate = randomTag(depth);
            if (candidate.getId() == tag.getId()) {
                return candidate;
            }
        }
        return tag.copy();
    }

    // ---------------------------------------------------------------- writing helpers

    static byte[] network(Tag tag) throws IOException {
        ByteBuf buf = Unpooled.buffer();
        NbtIo.writeAnyTag(tag, new ByteBufOutputStream(buf));
        byte[] bytes = new byte[buf.readableBytes()];
        buf.readBytes(bytes);
        return bytes;
    }

    static byte[] file(CompoundTag tag) throws IOException {
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        NbtIo.write(tag, new DataOutputStream(bytes));
        return bytes.toByteArray();
    }

    /** An exception as tokens: class, message, and for a ReportedNbtException its cause and details. */
    static List<String> thrown(Throwable e) {
        List<String> r = new ArrayList<>();
        r.add(u(e.getClass().getSimpleName()));
        r.add(e.getMessage() == null ? "z:0" : u(e.getMessage()));
        if (e instanceof ReportedException reported) {
            Throwable cause = reported.getCause();
            r.add(u(cause.getClass().getSimpleName()));
            r.add(cause.getMessage() == null ? "z:0" : u(cause.getMessage()));
            String name = "~none~";
            String type = "~none~";
            for (CrashReportCategory category : categories(reported.getReport())) {
                for (CrashReportCategory.Entry entry : entries(category)) {
                    if (entry.key().equals("Tag name")) {
                        name = entry.value();
                    } else if (entry.key().equals("Tag type")) {
                        type = entry.value();
                    }
                }
            }
            r.add(u(name));
            r.add(u(type));
        }
        return r;
    }

    @SuppressWarnings("unchecked")
    static List<CrashReportCategory> categories(CrashReport report) {
        try {
            Field f = CrashReport.class.getDeclaredField("details");
            f.setAccessible(true);
            return (List<CrashReportCategory>) f.get(report);
        } catch (ReflectiveOperationException e) {
            throw new IllegalStateException(e);
        }
    }

    @SuppressWarnings("unchecked")
    static List<CrashReportCategory.Entry> entries(CrashReportCategory category) {
        try {
            Field f = CrashReportCategory.class.getDeclaredField("entries");
            f.setAccessible(true);
            return (List<CrashReportCategory.Entry>) f.get(category);
        } catch (ReflectiveOperationException e) {
            throw new IllegalStateException(e);
        }
    }

    // ---------------------------------------------------------------- 1. tags

    /**
     * nbt.tag x:<packet bytes of a random tag> = x:<packet bytes of the re-read tag>
     * x:<file bytes or empty> u:<toString> u:<pretty> i:<hashCode> i:<sizeInBytes> x:<packet bytes of copy()>
     */
    private void tags() throws IOException {
        for (int i = 0; i < 60_000; i++) {
            Tag original = randomTag(0);
            if (i % 3 != 0) {
                CompoundTag root = new CompoundTag();
                root.put(randomKey(), original);
                if (rnd.nextBoolean()) {
                    root.putInt("DataVersion", 5023);
                }
                original = root;
            }
            byte[] input = network(original);
            Tag tag = NbtIo.readAnyTag(new DataInputStream(new ByteArrayInputStream(input)), NbtAccounter.unlimitedHeap());
            List<String> r = new ArrayList<>();
            r.add(x(network(tag)));
            r.add(tag instanceof CompoundTag compound ? x(file(compound)) : "x:");
            r.add(u(tag.toString()));
            r.add(u(new SnbtPrinterTagVisitor().visit(tag)));
            r.add("i:" + tag.hashCode());
            r.add("i:" + tag.sizeInBytes());
            r.add(x(network(tag.copy())));
            line("nbt.tag", List.of(x(input)), r);
        }
    }

    // ---------------------------------------------------------------- 2. malformed input

    private byte[] mutate(byte[] bytes) {
        byte[] b = bytes.clone();
        switch (rnd.nextInt(5)) {
            case 0 -> b = java.util.Arrays.copyOf(b, rnd.nextInt(b.length + 1));
            case 1 -> {
                for (int i = 0; i < 1 + rnd.nextInt(3) && b.length > 0; i++) {
                    b[rnd.nextInt(b.length)] = (byte) rnd.nextInt();
                }
            }
            case 2 -> {
                if (b.length > 1) {
                    b[rnd.nextInt(1, b.length)] = (byte) (rnd.nextBoolean() ? 0xFF : 0x80);
                }
            }
            case 3 -> {
                b = new byte[rnd.nextInt(24)];
                rnd.nextBytes(b);
                if (b.length > 0) {
                    b[0] = (byte) rnd.nextInt(14);
                }
            }
            default -> {
                // Deep nesting: lists of lists.
                int depth = rnd.nextInt(500, 530);
                ByteArrayOutputStream deep = new ByteArrayOutputStream();
                deep.write(9);
                for (int i = 0; i < depth; i++) {
                    deep.writeBytes(new byte[] {9, 0, 0, 0, 1});
                }
                deep.writeBytes(new byte[] {1, 0, 0, 0, 0});
                b = deep.toByteArray();
            }
        }
        return b;
    }

    private interface Reader {
        Tag read(DataInput input, NbtAccounter accounter) throws IOException;
    }

    /**
     * nbt.read i:<mode> l:<quota> i:<max depth> x:<bytes> = result... where mode is
     * 0 packet readAnyTag, 1 file readAnyTag, 2 file NbtIo.read, 3 file readUnnamedTag.
     * The result is x:<packet bytes> l:<usage> i:<depth> i:<consumed>, or the exception tokens.
     */
    private void malformed() throws IOException {
        for (int i = 0; i < 25_000; i++) {
            Tag tag = randomTag(0);
            byte[] valid;
            int mode = rnd.nextInt(4);
            if (mode >= 2) {
                CompoundTag root = new CompoundTag();
                root.put(randomKey(), tag);
                valid = file(root);
            } else {
                valid = network(tag);
            }
            byte[] bytes = rnd.nextInt(5) == 0 ? valid : mutate(valid);
            long quota = switch (rnd.nextInt(4)) {
                case 0 -> 2097152L;
                case 1 -> Long.MAX_VALUE;
                default -> rnd.nextLong(0, 400);
            };
            int maxDepth = rnd.nextInt(4) == 0 ? rnd.nextInt(0, 4) : 512;
            NbtAccounter accounter = new NbtAccounter(quota, maxDepth);
            List<String> r = new ArrayList<>();
            try {
                DataInput input;
                ByteBuf buf = null;
                ByteArrayInputStream stream = new ByteArrayInputStream(bytes);
                if (mode == 0) {
                    buf = Unpooled.wrappedBuffer(bytes);
                    input = new ByteBufInputStream(buf);
                } else {
                    input = new DataInputStream(stream);
                }
                Tag result = switch (mode) {
                    case 0, 1 -> NbtIo.readAnyTag(input, accounter);
                    case 2 -> NbtIo.read(input, accounter);
                    default -> readUnnamed(input, accounter);
                };
                int consumed = buf != null ? buf.readerIndex() : bytes.length - stream.available();
                r.add(x(network(result)));
                r.add("l:" + accounter.getUsage());
                r.add("i:" + accounter.getDepth());
                r.add("i:" + consumed);
            } catch (Exception e) {
                r = thrown(e);
            } catch (OutOfMemoryError e) {
                // A huge declared size under a large quota: Java's outcome depends on its heap.
                r = List.of(u("OutOfMemoryError"));
            }
            line("nbt.read", List.of("i:" + mode, "l:" + quota, "i:" + maxDepth, x(bytes)), r);
        }
    }

    private static Tag readUnnamed(DataInput input, NbtAccounter accounter) throws IOException {
        return NbtIo.readUnnamedTag(input, accounter);
    }

    // ---------------------------------------------------------------- 3. visitors

    /**
     * nbt.collect / nbt.skip x:<packet bytes of a compound> u:<selectors> = x:<result> [i:<missing>]
     * Selectors are "type:path/name" joined by '\u0001', paths joined by '/'.
     */
    private void visitors() throws IOException {
        for (int i = 0; i < 20_000; i++) {
            CompoundTag root = new CompoundTag();
            int n = rnd.nextInt(1, 8);
            for (int j = 0; j < n; j++) {
                root.put(randomKey(), rnd.nextInt(3) == 0 ? nested() : randomTag(2));
            }
            byte[] bytes = network(root);
            List<FieldSelector> selectors = new ArrayList<>();
            StringBuilder encoded = new StringBuilder();
            int count = rnd.nextInt(1, 4);
            for (int j = 0; j < count; j++) {
                List<String> path = new ArrayList<>();
                int depth = rnd.nextInt(3);
                for (int d = 0; d < depth; d++) {
                    path.add(KEYS[rnd.nextInt(6)]);
                }
                int type = rnd.nextInt(1, 13);
                String name = KEYS[rnd.nextInt(8)];
                selectors.add(new FieldSelector(path, TagTypes.getType(type), name));
                if (j > 0) {
                    encoded.append('\u0001');
                }
                encoded.append(type).append(':').append(String.join("/", path)).append('/').append(name);
            }
            FieldSelector[] array = selectors.toArray(FieldSelector[]::new);
            List<String> r = new ArrayList<>();
            try {
                CollectFields collect = new CollectFields(array);
                DataInput input = new DataInputStream(new ByteArrayInputStream(bytes, 1, bytes.length - 1));
                TagTypes.getType(bytes[0]).parseRoot(input, collect, NbtAccounter.unlimitedHeap());
                Tag result = collect.getResult();
                r.add(x(network(result == null ? EndTag.INSTANCE : result)));
                r.add("i:" + collect.getMissingFieldCount());
            } catch (Exception e) {
                r = thrown(e);
            }
            line("nbt.collect", List.of(x(bytes), u(encoded.toString())), r);
            SkipFields skip = new SkipFields(array);
            root.acceptAsRoot(skip);
            Tag skipped = skip.getResult();
            line("nbt.skip", List.of(x(bytes), u(encoded.toString())),
                List.of(x(network(skipped == null ? EndTag.INSTANCE : skipped))));
        }
    }

    /** A compound nested under the first few KEYS, so selectors with paths find something. */
    private CompoundTag nested() {
        CompoundTag inner = new CompoundTag();
        for (int j = 0; j < rnd.nextInt(1, 6); j++) {
            String key = KEYS[rnd.nextInt(8)];
            inner.put(key, rnd.nextInt(3) == 0 ? nested() : randomTag(3));
        }
        return inner;
    }

    // ---------------------------------------------------------------- 4. SNBT

    private static final String[] SNBT = {
        "", " ", "1", "1b", "1B", "-1s", "1i", "1L", "1ub", "1us", "1ui", "1ul", "1sb", "-1sb", "1u",
        "0", "00", "012", "0x", "0xFF", "0xffub", "-0x1", "0b", "0b101", "0B11sb", "0x1_F", "1_000", "_1", "1_",
        "1.", ".5", "1.5", "1.5f", "1.5F", "1.5d", "1e3", "1E-3", "1e3f", "1e999", "1e999f", "-0.0", "1.e2",
        "1.5e", "1f", "1d", "2147483648", "-2147483648", "9223372036854775808L", "128b", "255ub", "256ub",
        "65535us", "true", "FALSE", "tRuE", "abc", "a.b+c-d_e", "+", "-", ".", "+abc", "'x'", "\"x\"", "''",
        "\"\"", "'it\\'s'", "\"a\\\"b\"", "\"\\n\\t\\b\\f\\r\\s\\\\\"", "'\\x41\\u00e9\\U0001F600'",
        "\"\\N{SNOWMAN}\"", "\"\\N{snowman}\"", "\"\\N{ NOT A NAME }\"", "\"\\N{}\"", "\"\\uD800\"",
        "\"\\U00110000\"", "\"\\xZZ\"", "\"\\q\"", "\"unterminated", "{}", "{ }", "{a:1}", "{a:1,}", "{a:1,b:2}",
        "{a:1,a:2}", "{\"\":1}", "{'k':1}", "{a b:1}", "{a:}", "{:1}", "{a:1", "[]", "[,]", "[1,2,3]", "[1,'a']",
        "[1,]", "[B;]", "[B;1b,2b]", "[B;1]", "[B;1s]", "[I;1,2]", "[I;1b,2s]", "[I;1L]", "[L;1,2L]",
        "[L;1.5]", "[X;1]", "[B;1b", "bool(1)", "bool(0.0)", "bool(true)", "bool('x')", "bool()",
        "bool(1,2)", "uuid('f81d4fae-7dec-11d0-a765-00a0c91e6bf6')", "uuid(\"1-2-3-4-5\")", "uuid(1)",
        "uuid('nope')", "nope(1)", "Bool(1)", "{a:[{b:1b,c:'d'},{e:[I;1,2]}],f:\"g\"}", "{a:1} x", "1 2",
        "  {  a  :  1  }  ", "{a:\t1}", "\u3000{a:1}", "[1, 2,]", "{a:[]}", "[[],[]]", "[{},{}]",
    };

    private String mutateSnbt(String s) {
        String alphabet = "{}[]:,;'\"\\ .-+_0123456789abefilsuxBILNUtrue";
        StringBuilder b = new StringBuilder(s);
        int edits = 1 + rnd.nextInt(3);
        for (int i = 0; i < edits; i++) {
            int pos = b.isEmpty() ? 0 : rnd.nextInt(b.length() + 1);
            switch (rnd.nextInt(3)) {
                case 0 -> b.insert(pos, alphabet.charAt(rnd.nextInt(alphabet.length())));
                case 1 -> {
                    if (pos < b.length()) {
                        b.deleteCharAt(pos);
                    }
                }
                default -> {
                    if (pos < b.length()) {
                        b.setCharAt(pos, alphabet.charAt(rnd.nextInt(alphabet.length())));
                    }
                }
            }
        }
        return b.toString();
    }

    /**
     * snbt.parse u:<input> = x:<packet bytes>, or s:<translation key> i:<cursor> and the
     * arguments (u: strings, i:/l:/d:/f: numbers), or the exception tokens.
     * snbt.compound u:<input> is the same through TagParser.parseCompoundFully.
     */
    private void snbt() throws IOException {
        TagParser<Tag> parser = TagParser.create(NbtOps.INSTANCE);
        List<String> inputs = new ArrayList<>(List.of(SNBT));
        for (int i = 0; i < 30_000; i++) {
            inputs.add(randomTag(1).toString());
        }
        for (int i = 0; i < 90_000; i++) {
            String base = rnd.nextInt(3) == 0 ? SNBT[rnd.nextInt(SNBT.length)] : randomTag(2).toString();
            inputs.add(mutateSnbt(base));
        }
        for (String input : inputs) {
            line("snbt.parse", List.of(u(input)), parseResult(() -> parser.parseFully(input)));
            if (input.startsWith("{") || rnd.nextInt(10) == 0) {
                line("snbt.compound", List.of(u(input)), parseResult(() -> TagParser.parseCompoundFully(input)));
            }
        }
    }

    private interface Parse {
        Tag run() throws CommandSyntaxException;
    }

    private static List<String> parseResult(Parse parse) throws IOException {
        try {
            return List.of(x(network(parse.run())));
        } catch (CommandSyntaxException e) {
            List<String> r = new ArrayList<>();
            Component message = (Component) e.getRawMessage();
            if (message.getContents() instanceof TranslatableContents translatable) {
                r.add("s:" + HEX.formatHex(translatable.getKey().getBytes(StandardCharsets.UTF_8)));
                r.add("i:" + e.getCursor());
                for (Object arg : translatable.getArgs()) {
                    r.add(switch (arg) {
                        case String s -> u(s);
                        case Integer v -> "i:" + v;
                        case Long v -> "l:" + v;
                        case Float v -> "f:" + HEX.toHexDigits(Float.floatToRawIntBits(v));
                        case Double v -> "d:" + HEX.toHexDigits(Double.doubleToRawLongBits(v));
                        default -> u("?" + arg.getClass().getSimpleName() + ":" + arg);
                    });
                }
            } else {
                r.add(u("literal:" + message.getString()));
            }
            return r;
        } catch (RuntimeException e) {
            return thrown(e);
        }
    }
}

/*
 * CeriumMC Java standard library recorder. Copyright StepByte Studios, GPL-2.0-only.
 *
 * Records the JDK behaviour that cerium-java reproduces: HashMap iteration order after random
 * operation scripts, Double/Float.toString, the number parsers and their exception messages,
 * Character.isWhitespace/digit/getName/codePointOf, UUID.fromString, Modified UTF-8, and
 * String.hashCode/compareTo. Strings are written as UTF-16 code units (u: tokens), so lone
 * surrogates survive. Where Java throws, the result is the exception's message as a u: token
 * (prefixed by the class's simple name and ": "). It uses only the JDK, no Minecraft code.
 *
 * Usage: java -cp <classes> golden.JStdMain <output-dir>
 */
package golden;

import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.BufferedWriter;
import java.io.DataInputStream;
import java.io.DataOutputStream;
import java.io.IOException;
import java.math.BigDecimal;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HexFormat;
import java.util.List;
import java.util.Map;
import java.util.SplittableRandom;
import java.util.UUID;

public final class JStdMain {
    private static final HexFormat HEX = HexFormat.of();
    private final SplittableRandom rnd = new SplittableRandom(0x7A7A_0003L);
    private final BufferedWriter out;
    private long cases;

    private JStdMain(BufferedWriter out) {
        this.out = out;
    }

    public static void main(String[] args) throws Exception {
        Path dir = Path.of(args[0]);
        Files.createDirectories(dir);
        try (BufferedWriter out = Files.newBufferedWriter(dir.resolve("jstd.txt"), StandardCharsets.UTF_8)) {
            JStdMain main = new JStdMain(out);
            main.hashMaps();
            main.toStrings();
            main.floatingParsers();
            main.integerParsers();
            main.characters();
            main.uuids();
            main.modifiedUtf8();
            main.strings();
            System.out.println("jstd.txt: " + main.cases + " cases");
        }
    }

    private interface Call {
        String run() throws Exception;
    }

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

    private static String error(Throwable e) {
        return u(e.getClass().getSimpleName() + ": " + e.getMessage());
    }

    private void rec(String op, Call call, String... args) throws IOException {
        String result;
        try {
            result = call.run();
        } catch (Exception e) {
            result = error(e);
        }
        StringBuilder line = new StringBuilder(op);
        for (String a : args) {
            line.append(' ').append(a);
        }
        line.append(" = ").append(result).append('\n');
        out.write(line.toString());
        cases++;
    }

    // ---------------------------------------------------------------- HashMap

    /** Keys: short words, numbers, and families that share one hash code (to build trees). */
    private List<String> keyPool() {
        List<String> keys = new ArrayList<>();
        String[] words = {"pos", "state", "nbt", "DataVersion", "id", "Count", "tag", "x", "y",
            "z", "Name", "Properties", "palette", "blocks", "entities", "size", "data", ""};
        keys.addAll(List.of(words));
        for (int i = 0; i < 300; i++) {
            keys.add(Integer.toString(i));
            keys.add("k" + Integer.toHexString(rnd.nextInt()));
        }
        // 2^6 strings over {"Aa", "BB"} share a hash; so do the "C#"/"Bb" variants.
        for (String[] pair : new String[][] {{"Aa", "BB"}, {"C#", "Bb"}}) {
            for (int bits = 0; bits < 64; bits++) {
                StringBuilder b = new StringBuilder();
                for (int j = 0; j < 6; j++) {
                    b.append(pair[(bits >> j) & 1]);
                }
                keys.add(b.toString());
            }
        }
        keys.add("\uD800");
        keys.add("été");
        keys.add("😀");
        return keys;
    }

    private void hashMaps() throws IOException {
        List<String> pool = keyPool();
        StringBuilder poolArg = new StringBuilder();
        for (int i = 0; i < pool.size(); i++) {
            if (i > 0) {
                poolArg.append('\u0001');
            }
            poolArg.append(pool.get(i));
        }
        String poolToken = u(poolArg.toString());
        for (int scriptNo = 0; scriptNo < 3000; scriptNo++) {
            int length = 1 + rnd.nextInt(scriptNo % 10 == 0 ? 3000 : 200);
            // Bias some scripts to the colliding families so that bins become trees.
            boolean colliding = rnd.nextInt(3) == 0;
            int initial = rnd.nextInt(4) == 0 ? rnd.nextInt(100) : -1;
            HashMap<String, Integer> map = initial >= 0 ? new HashMap<>(initial) : new HashMap<>();
            StringBuilder script = new StringBuilder();
            script.append(initial).append(';');
            for (int step = 0; step < length; step++) {
                int key = colliding && rnd.nextBoolean()
                    ? pool.size() - 3 - 128 + rnd.nextInt(128)
                    : rnd.nextInt(pool.size());
                String k = pool.get(key);
                int value = rnd.nextInt(1000);
                int op = rnd.nextInt(100);
                char code;
                if (op < 50) {
                    code = 'P';
                    map.put(k, value);
                } else if (op < 62) {
                    code = 'R';
                    map.remove(k);
                } else if (op < 68) {
                    code = 'A';
                    map.putIfAbsent(k, value);
                } else if (op < 76) {
                    code = 'C';
                    map.computeIfAbsent(k, kk -> value);
                } else if (op < 82) {
                    code = 'M';
                    map.compute(k, (kk, old) -> {
                        if (old == null) {
                            return value;
                        }
                        Integer sum = old + value;
                        return sum % 3 == 0 ? null : sum;
                    });
                } else if (op < 88) {
                    code = 'G';
                    map.merge(k, value, (a, b) -> {
                        Integer sum = a + b;
                        return sum % 4 == 0 ? null : sum;
                    });
                } else if (op < 92) {
                    code = 'F';
                    int modulus = 2 + value % 5;
                    map.entrySet().removeIf(e -> e.getValue() % modulus == 0);
                } else if (op < 95) {
                    code = 'N';
                    map = new HashMap<>(map);
                } else if (op < 97) {
                    // putAll of a two-put map: L<key>:<value>:<second key>
                    int second = rnd.nextInt(pool.size());
                    HashMap<String, Integer> other = new HashMap<>();
                    other.put(k, value);
                    other.put(pool.get(second), value + 1);
                    script.append('L').append(key).append(':').append(value).append(':').append(second).append(';');
                    map.putAll(other);
                    continue;
                } else if (op < 98) {
                    code = 'X';
                    map.clear();
                } else {
                    code = 'K';
                    map = copyByPut(map);
                }
                script.append(code).append(key).append(':').append(value).append(';');
            }
            StringBuilder order = new StringBuilder();
            for (Map.Entry<String, Integer> e : map.entrySet()) {
                order.append(pool.indexOf(e.getKey())).append('=').append(e.getValue()).append(',');
            }
            final String result = "s:" + HEX.formatHex(order.toString().getBytes(StandardCharsets.UTF_8));
            rec("jstd.map", () -> result, poolToken,
                "s:" + HEX.formatHex(script.toString().getBytes(StandardCharsets.UTF_8)));
            poolToken = "z:0"; // the pool is sent once; later cases reuse it
        }
    }

    /** CompoundTag.copy(): a new default HashMap filled by put in iteration order. */
    private static HashMap<String, Integer> copyByPut(HashMap<String, Integer> map) {
        HashMap<String, Integer> copy = new HashMap<>();
        map.forEach(copy::put);
        return copy;
    }

    // ---------------------------------------------------------------- toString

    private double randomDouble() {
        return switch (rnd.nextInt(6)) {
            case 0 -> Double.longBitsToDouble(rnd.nextLong());
            case 1 -> rnd.nextDouble() * Math.pow(10, rnd.nextInt(40) - 20);
            case 2 -> (double) rnd.nextInt(2_000_000) / Math.pow(10, rnd.nextInt(12));
            case 3 -> Double.longBitsToDouble(rnd.nextLong(0, 0x0010_0000_0000_0000L));
            case 4 -> Math.scalb(1.0, rnd.nextInt(-1074, 1024)) * (rnd.nextBoolean() ? 1 : -1);
            default -> (float) Double.longBitsToDouble(rnd.nextLong());
        };
    }

    private float randomFloat() {
        return switch (rnd.nextInt(5)) {
            case 0 -> Float.intBitsToFloat(rnd.nextInt());
            case 1 -> (float) (rnd.nextDouble() * Math.pow(10, rnd.nextInt(20) - 10));
            case 2 -> (float) rnd.nextInt(2_000_000) / (float) Math.pow(10, rnd.nextInt(8));
            case 3 -> Float.intBitsToFloat(rnd.nextInt(0, 0x0080_0000));
            default -> Math.scalb(1.0f, rnd.nextInt(-149, 128));
        };
    }

    private void toStrings() throws IOException {
        for (int i = 0; i < 250_000; i++) {
            double v = randomDouble();
            rec("jstd.d2s", () -> u(Double.toString(v)), StdTokens.d(v));
            float f = randomFloat();
            rec("jstd.f2s", () -> u(Float.toString(f)), StdTokens.f(f));
        }
        double[] specials = {0.0, -0.0, Double.MIN_VALUE, Double.MIN_NORMAL, Double.MAX_VALUE,
            Double.NaN, Double.POSITIVE_INFINITY, Double.NEGATIVE_INFINITY, 1e7, 1e-3, 9999999.999999998,
            0.001, 0.0009999999999999998, 1e23, 2e23, 5e-324, 1.0E22, 4.35, 0.3};
        for (double v : specials) {
            rec("jstd.d2s", () -> u(Double.toString(v)), StdTokens.d(v));
            rec("jstd.f2s", () -> u(Float.toString((float) v)), StdTokens.f((float) v));
        }
    }

    // ---------------------------------------------------------------- parseDouble/parseFloat

    private String randomDecimal() {
        StringBuilder b = new StringBuilder();
        switch (rnd.nextInt(10)) {
            case 0 -> b.append(Double.toString(randomDouble()));
            case 1 -> b.append(Float.toString(randomFloat()));
            case 2 -> {
                // An exact halfway point between two doubles or floats.
                if (rnd.nextBoolean()) {
                    double v = Math.abs(randomDouble());
                    if (Double.isFinite(v) && v < Double.MAX_VALUE) {
                        b.append(new BigDecimal(v).add(new BigDecimal(Math.nextUp(v))).divide(BigDecimal.TWO).toString());
                    } else {
                        b.append("1.5");
                    }
                } else {
                    float v = Math.abs(randomFloat());
                    if (Float.isFinite(v) && v < Float.MAX_VALUE) {
                        b.append(new BigDecimal(v).add(new BigDecimal(Math.nextUp(v))).divide(BigDecimal.TWO).toPlainString());
                    } else {
                        b.append("2.5");
                    }
                }
            }
            case 3 -> {
                b.append("0x").append(Long.toHexString(rnd.nextLong()));
                if (rnd.nextBoolean()) {
                    b.insert(2 + rnd.nextInt(b.length() - 2), '.');
                }
                b.append(rnd.nextBoolean() ? 'p' : 'P').append(rnd.nextInt(-1200, 1200));
            }
            case 4 -> {
                // Many digits.
                int n = 1 + rnd.nextInt(rnd.nextInt(10) == 0 ? 800 : 60);
                for (int i = 0; i < n; i++) {
                    b.append((char) ('0' + rnd.nextInt(10)));
                }
                b.insert(rnd.nextInt(b.length() + 1), '.');
                if (rnd.nextBoolean()) {
                    b.append('e').append(rnd.nextInt(-700, 700));
                }
            }
            default -> {
                // Random characters from the syntax's alphabet.
                String alphabet = "0123456789.eE+-xXpPfFdDINaty ";
                int n = 1 + rnd.nextInt(12);
                for (int i = 0; i < n; i++) {
                    b.append(alphabet.charAt(rnd.nextInt(alphabet.length())));
                }
            }
        }
        if (rnd.nextInt(10) == 0) {
            b.insert(0, rnd.nextBoolean() ? '-' : '+');
        }
        if (rnd.nextInt(20) == 0) {
            b.insert(0, " \t");
        }
        if (rnd.nextInt(20) == 0) {
            b.append(rnd.nextBoolean() ? "f " : "D");
        }
        return b.toString();
    }

    private void floatingParsers() throws IOException {
        String[] fixed = {"", " ", "NaN", "-NaN", "Infinity", "+Infinity", "-Infinity ", "Infinityx",
            "1", "1.", ".1", ".", "-.", "1e", "1e+", "1e5", "1E-5", "1..2", "0x", "0x1", "0x1p", "0x1p1",
            "0x.8p1", "0X1P-1074", "0x1p-1075", "0x1.8p-1075", "1e-400", "1e400", "4.9e-324", "2.4703282292062328e-324",
            "2.4703282292062327e-324", "1.7976931348623158e308", "3.4028235677973366e38", "1.401298464324817e-45",
            "0.7006492321624085e-45", "0001.0000e0001", "1_0", "١", "1\u0000", "\u00001", "1d", "1fd"};
        for (String s : fixed) {
            rec("jstd.parse_d", () -> StdTokens.d(Double.parseDouble(s)), u(s));
            rec("jstd.parse_f", () -> StdTokens.f(Float.parseFloat(s)), u(s));
        }
        String longJunk = "1".repeat(1200) + "x";
        rec("jstd.parse_d", () -> StdTokens.d(Double.parseDouble(longJunk)), u(longJunk));
        for (int i = 0; i < 100_000; i++) {
            String s = randomDecimal();
            rec("jstd.parse_d", () -> StdTokens.d(Double.parseDouble(s)), u(s));
            String t = randomDecimal();
            rec("jstd.parse_f", () -> StdTokens.f(Float.parseFloat(t)), u(t));
        }
    }

    // ---------------------------------------------------------------- integer parsers

    private String randomIntegerString() {
        String alphabet = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFXYZ+-_ ١१１Ａ";
        StringBuilder b = new StringBuilder();
        switch (rnd.nextInt(4)) {
            case 0 -> b.append(rnd.nextInt());
            case 1 -> b.append(Long.toString(rnd.nextLong(), 2 + rnd.nextInt(35)));
            case 2 -> b.append(Long.toUnsignedString(rnd.nextLong(), 2 + rnd.nextInt(35)));
            default -> {
                int n = rnd.nextInt(22);
                for (int i = 0; i < n; i++) {
                    b.append(alphabet.charAt(rnd.nextInt(alphabet.length())));
                }
            }
        }
        return b.toString();
    }

    private void integerParsers() throws IOException {
        String[] fixed = {"", "+", "-", "0", "-0", "+0", "2147483647", "2147483648", "-2147483648",
            "-2147483649", "4294967295", "4294967296", "9223372036854775807", "9223372036854775808",
            "-9223372036854775808", "18446744073709551615", "18446744073709551616", "127", "128", "-128",
            "-129", "255", "256", "32767", "32768", "65535", "65536", "ff", "FF", "7fffffff", "80000000",
            "ffffffff", "100000000", "ffffffffffffffff", "10000000000000000"};
        for (int pass = 0; pass < 25_000 + fixed.length; pass++) {
            String s = pass < fixed.length ? fixed[pass] : randomIntegerString();
            int radix = pass < fixed.length
                ? new int[] {10, 16, 2, 36}[pass % 4]
                : (rnd.nextInt(30) == 0 ? rnd.nextInt(40) : new int[] {2, 10, 16, 36, 8, 7}[rnd.nextInt(6)]);
            String r = "i:" + radix;
            rec("jstd.parse_i", () -> "i:" + Integer.parseInt(s, radix), u(s), r);
            rec("jstd.parse_l", () -> "l:" + Long.parseLong(s, radix), u(s), r);
            rec("jstd.parse_ui", () -> "i:" + Integer.parseUnsignedInt(s, radix), u(s), r);
            rec("jstd.parse_ul", () -> "l:" + Long.parseUnsignedLong(s, radix), u(s), r);
            rec("jstd.parse_b", () -> "b:" + Byte.parseByte(s, radix), u(s), r);
            rec("jstd.parse_s", () -> "i:" + Short.parseShort(s, radix), u(s), r);
            int begin = s.isEmpty() ? 0 : rnd.nextInt(s.length() + 1);
            int end = begin + rnd.nextInt(s.length() - begin + 1);
            rec("jstd.parse_lr", () -> "l:" + Long.parseLong(s, begin, end, radix), u(s), "i:" + begin,
                "i:" + end, r);
        }
    }

    // ---------------------------------------------------------------- Character

    private void characters() throws IOException {
        for (int c = 0; c <= 0xFFFF; c++) {
            char ch = (char) c;
            rec("jstd.ws", () -> "z:" + (Character.isWhitespace(ch) ? 1 : 0), "i:" + c);
            int radix = 2 + c % 35;
            rec("jstd.digit", () -> "i:" + Character.digit(ch, radix), "i:" + c, "i:" + radix);
        }
        for (int cp = 0; cp <= Character.MAX_CODE_POINT; cp++) {
            String name = Character.getName(cp);
            if (name != null) {
                rec("jstd.name", () -> u(name), "i:" + cp);
            } else if (cp % 97 == 0) {
                rec("jstd.name", () -> "z:0", "i:" + cp);
            }
        }
        for (int i = 0; i < 40_000; i++) {
            int cp = rnd.nextInt(0x110000);
            String name = Character.getName(cp);
            String query = name == null ? "UNKNOWN " + Integer.toHexString(cp) : name;
            query = mangleName(query);
            String q = query;
            rec("jstd.cp_of", () -> "i:" + Character.codePointOf(q), u(q));
        }
    }

    private String mangleName(String name) {
        StringBuilder b = new StringBuilder();
        for (int i = 0; i < name.length(); i++) {
            char c = name.charAt(i);
            int roll = rnd.nextInt(60);
            if (roll < 20) {
                b.append(Character.toLowerCase(c));
            } else if (roll == 20 && c == 'I') {
                b.append('ı');
            } else if (roll == 21 && c == 'S') {
                b.append('ſ');
            } else if (roll == 22) {
                b.append('é');
            } else if (roll == 23 && i + 1 < name.length() && name.startsWith("SS", i)) {
                b.append('ß');
                i++;
            } else if (roll == 24 && i + 1 < name.length() && name.startsWith("FI", i)) {
                b.append('ﬁ');
                i++;
            } else if (roll == 25) {
                // Drop a character.
            } else {
                b.append(c);
            }
        }
        if (rnd.nextInt(10) == 0) {
            b.insert(0, "  ");
        }
        if (rnd.nextInt(10) == 0) {
            b.append("\t");
        }
        if (rnd.nextInt(20) == 0) {
            int space = b.lastIndexOf(" ");
            if (space >= 0) {
                b.insert(space + 1, rnd.nextBoolean() ? "+" : "0");
            }
        }
        return b.toString();
    }

    // ---------------------------------------------------------------- UUID

    private void uuids() throws IOException {
        String alphabet = "0123456789abcdefABCDEF-+ xg١";
        for (int i = 0; i < 30_000; i++) {
            String s;
            switch (rnd.nextInt(4)) {
                case 0 -> s = new UUID(rnd.nextLong(), rnd.nextLong()).toString();
                case 1 -> {
                    StringBuilder b = new StringBuilder(new UUID(rnd.nextLong(), rnd.nextLong()).toString());
                    b.setCharAt(rnd.nextInt(b.length()), alphabet.charAt(rnd.nextInt(alphabet.length())));
                    s = b.toString();
                }
                case 2 -> {
                    StringBuilder b = new StringBuilder();
                    for (int g = 0; g < 4 + rnd.nextInt(3); g++) {
                        if (g > 0) {
                            b.append('-');
                        }
                        b.append(Long.toHexString(rnd.nextLong()), 0, rnd.nextInt(14));
                    }
                    s = b.toString();
                }
                default -> {
                    StringBuilder b = new StringBuilder();
                    int n = rnd.nextInt(40);
                    for (int j = 0; j < n; j++) {
                        b.append(alphabet.charAt(rnd.nextInt(alphabet.length())));
                    }
                    s = b.toString();
                }
            }
            String input = s;
            rec("jstd.uuid", () -> {
                UUID uuid = UUID.fromString(input);
                return "l:" + uuid.getMostSignificantBits() + " l:" + uuid.getLeastSignificantBits();
            }, u(input));
        }
    }

    // ---------------------------------------------------------------- Modified UTF-8

    private String randomJavaString(int maxLen) {
        StringBuilder b = new StringBuilder();
        int n = rnd.nextInt(maxLen + 1);
        for (int i = 0; i < n; i++) {
            b.append(switch (rnd.nextInt(5)) {
                case 0 -> (char) rnd.nextInt(0x80);
                case 1 -> (char) rnd.nextInt(0x800);
                case 2 -> (char) (0xD800 + rnd.nextInt(0x800));
                case 3 -> (char) 0;
                default -> (char) rnd.nextInt(0x10000);
            });
        }
        return b.toString();
    }

    private void modifiedUtf8() throws IOException {
        for (int i = 0; i < 50_000; i++) {
            String s = randomJavaString(i % 1000 == 0 ? 30_000 : 40);
            rec("jstd.mutf8_enc", () -> {
                ByteArrayOutputStream bytes = new ByteArrayOutputStream();
                new DataOutputStream(bytes).writeUTF(s);
                return x(bytes.toByteArray());
            }, u(s));
            byte[] raw = new byte[rnd.nextInt(24)];
            rnd.nextBytes(raw);
            if (rnd.nextBoolean()) {
                for (int j = 0; j < raw.length; j++) {
                    raw[j] = (byte) (raw[j] & (rnd.nextBoolean() ? 0x7F : 0xEF));
                }
            }
            byte[] framed = new byte[raw.length + 2];
            framed[1] = (byte) raw.length;
            System.arraycopy(raw, 0, framed, 2, raw.length);
            rec("jstd.mutf8_dec", () -> u(new DataInputStream(new ByteArrayInputStream(framed)).readUTF()), x(raw));
        }
    }

    // ---------------------------------------------------------------- String

    private void strings() throws IOException {
        for (int i = 0; i < 50_000; i++) {
            String a = randomJavaString(6);
            String b = rnd.nextInt(4) == 0 ? a + randomJavaString(2) : randomJavaString(6);
            rec("jstd.str", () -> "i:" + a.hashCode() + " i:" + Integer.signum(a.compareTo(b)), u(a), u(b));
        }
    }
}

/** Shared bit-exact float tokens. */
final class StdTokens {
    private static final HexFormat HEX = HexFormat.of();

    private StdTokens() {
    }

    static String f(float v) {
        return "f:" + HEX.toHexDigits(Float.floatToRawIntBits(v));
    }

    static String d(double v) {
        return "d:" + HEX.toHexDigits(Double.doubleToRawLongBits(v));
    }
}

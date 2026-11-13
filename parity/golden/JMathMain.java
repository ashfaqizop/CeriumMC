/*
 * CeriumMC java.lang.Math recorder. Copyright StepByte Studios, GPL-2.0-only.
 *
 * Records HotSpot's results for the Math functions that are CPU intrinsics on x86_64 (Intel
 * LIBM: sin, cos, tan, log, log10, exp, pow) and for StrictMath-backed ones, in the golden
 * file format. It uses only the JDK, no Minecraft code. It also prints a SHA-256 digest of
 * each function's results, so runs on different CPUs (Intel vs AMD, x86_64 vs ARM) can be
 * compared directly.
 *
 * Usage: java -cp <classes> golden.JMathMain <output-dir>
 */
package golden;

import java.io.BufferedWriter;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.HexFormat;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.SplittableRandom;
import java.util.function.DoubleBinaryOperator;
import java.util.function.DoubleUnaryOperator;

public final class JMathMain {
    private static final HexFormat HEX = HexFormat.of();
    private static final int N = 200_000;

    private final SplittableRandom rnd = new SplittableRandom(0x7A7A_0001L);
    private final BufferedWriter out;
    private final Map<String, MessageDigest> digests = new LinkedHashMap<>();
    private long cases;

    private JMathMain(BufferedWriter out) {
        this.out = out;
    }

    public static void main(String[] args) throws Exception {
        Path dir = Path.of(args[0]);
        Files.createDirectories(dir);
        try (BufferedWriter out = Files.newBufferedWriter(dir.resolve("jmath.txt"), StandardCharsets.UTF_8)) {
            JMathMain main = new JMathMain(out);
            main.run();
            System.out.println("jmath.txt: " + main.cases + " cases");
            for (Map.Entry<String, MessageDigest> e : main.digests.entrySet()) {
                System.out.println("digest " + e.getKey() + " " + HEX.formatHex(e.getValue().digest()));
            }
        }
        System.out.println("cpu " + System.getProperty("os.arch") + " "
            + System.getProperty("java.vm.name") + " " + System.getProperty("java.vm.version"));
    }

    private static String bits(double v) {
        return HEX.toHexDigits(Double.doubleToRawLongBits(v));
    }

    private void record(String op, double result, double... args) throws IOException {
        StringBuilder line = new StringBuilder(op);
        for (double a : args) line.append(" d:").append(bits(a));
        line.append(" = d:").append(bits(result)).append('\n');
        out.write(line.toString());
        cases++;
        MessageDigest md = digests.computeIfAbsent(op, k -> {
            try {
                return MessageDigest.getInstance("SHA-256");
            } catch (Exception e) {
                throw new IllegalStateException(e);
            }
        });
        md.update(line.toString().getBytes(StandardCharsets.UTF_8));
    }

    private void unary(String op, DoubleUnaryOperator f, double x) throws IOException {
        record(op, f.applyAsDouble(x), x);
    }

    private void binary(String op, DoubleBinaryOperator f, double x, double y) throws IOException {
        record(op, f.applyAsDouble(x, y), x, y);
    }

    /** Any double: a random bit pattern. */
    private double anyBits() {
        return Double.longBitsToDouble(rnd.nextLong());
    }

    private static final double[] SPECIAL = {
        0.0, -0.0, 1.0, -1.0, 0.5, 2.0, 10.0, 0.1, Math.E, Math.PI, Math.PI / 2, Math.PI / 4,
        Double.MIN_VALUE, Double.MIN_NORMAL, Double.MAX_VALUE, Double.POSITIVE_INFINITY,
        Double.NEGATIVE_INFINITY, Double.NaN, 1.0000000000000002, 0.9999999999999999,
        709.782712893384, -745.1332191019411, 1e-300, 1e300, 3.0, 0.25, 1e22, 1e-22,
    };

    private void run() throws IOException {
        for (double x : SPECIAL) {
            unary("jm.log", Math::log, x);
            unary("jm.log10", Math::log10, x);
            unary("jm.exp", Math::exp, x);
            unary("jm.sin", Math::sin, x);
            unary("jm.cos", Math::cos, x);
            unary("jm.tan", Math::tan, x);
            for (double y : SPECIAL) binary("jm.pow", Math::pow, x, y);
        }
        for (int i = 0; i < N; i++) {
            // log: the Gaussian's domain (0, 1), around 1, and any positive double.
            double u = rnd.nextDouble();
            unary("jm.log", Math::log, u == 0.0 ? 0.5 : u);
            unary("jm.log", Math::log, 1.0 + rnd.nextDouble(-1e-3, 1e-3));
            unary("jm.log", Math::log, Math.abs(anyBits()));
            unary("jm.log10", Math::log10, Math.abs(anyBits()));
            unary("jm.log10", Math::log10, rnd.nextDouble(0.0, 1e6));
            unary("jm.exp", Math::exp, rnd.nextDouble(-750.0, 715.0));
            unary("jm.exp", Math::exp, rnd.nextDouble(-1.0, 1.0));
            unary("jm.sin", Math::sin, rnd.nextDouble(-7.0, 7.0));
            unary("jm.sin", Math::sin, rnd.nextDouble(-1e6, 1e6));
            unary("jm.sin", Math::sin, anyBits());
            unary("jm.cos", Math::cos, rnd.nextDouble(-7.0, 7.0));
            unary("jm.cos", Math::cos, rnd.nextDouble(-1e6, 1e6));
            unary("jm.cos", Math::cos, anyBits());
            unary("jm.tan", Math::tan, rnd.nextDouble(-7.0, 7.0));
            unary("jm.tan", Math::tan, anyBits());
            // pow: physics-style bases and exponents, integers, halves, and any doubles.
            binary("jm.pow", Math::pow, rnd.nextDouble(0.0, 4.0), rnd.nextDouble(-10.0, 10.0));
            binary("jm.pow", Math::pow, rnd.nextDouble(-100.0, 100.0), rnd.nextInt(-20, 21));
            binary("jm.pow", Math::pow, rnd.nextDouble(0.0, 100.0), rnd.nextInt(-8, 9) * 0.5);
            binary("jm.pow", Math::pow, anyBits(), anyBits());
        }
        // StrictMath-backed functions that vanilla calls (Java's pure-Java FdLibm port): atan2,
        // asin, acos and log1p. CeriumMC uses the libm crate for these and must match bit for bit.
        for (double x : SPECIAL) {
            unary("jm.asin", Math::asin, x);
            unary("jm.acos", Math::acos, x);
            unary("jm.log1p", Math::log1p, x);
            for (double y : SPECIAL) binary("jm.atan2", Math::atan2, x, y);
        }
        for (int i = 0; i < N; i++) {
            unary("jm.asin", Math::asin, rnd.nextDouble(-1.0, 1.0));
            unary("jm.acos", Math::acos, rnd.nextDouble(-1.0, 1.0));
            unary("jm.asin", Math::asin, anyBits());
            unary("jm.log1p", Math::log1p, rnd.nextDouble(-1.0, 100.0));
            unary("jm.log1p", Math::log1p, anyBits());
            binary("jm.atan2", Math::atan2, rnd.nextDouble(-10.0, 10.0), rnd.nextDouble(-10.0, 10.0));
            binary("jm.atan2", Math::atan2, anyBits(), anyBits());
        }
        // log, log10, pow: a dense sweep over the mantissa. The stubs' RCPPS step reads the top
        // 23 mantissa bits, and Intel and AMD differ for about 1.6% of them, so each of the 2^19
        // buckets of 16 such values gets one case per function, at a random scale.
        for (long bucket = 0; bucket < (1L << 19); bucket++) {
            long mantissa = (bucket << 33) | (rnd.nextLong() & ((1L << 33) - 1));
            double unit = Double.longBitsToDouble(0x3FF0_0000_0000_0000L | mantissa);
            double x = Math.scalb(unit, rnd.nextInt(-40, 41));
            unary("jm.log", Math::log, x);
            unary("jm.log10", Math::log10, x);
            binary("jm.pow", Math::pow, x, rnd.nextDouble(-4.0, 4.0));
        }
    }
}

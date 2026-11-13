/*
 * CeriumMC java.lang.Math helper recorder. Copyright StepByte Studios, GPL-2.0-only.
 *
 * Records the results of the simple java.lang.Math methods vanilla calls (round, signum,
 * copySign, toRadians, toDegrees, nextUp, nextDown, clamp and the *Exact family) in the golden
 * file format. Where Java throws, the result is the exception's simple class name as a string.
 * It uses only the JDK, no Minecraft code.
 *
 * Usage: java -cp <classes> golden.JLangMain <output-dir>
 */
package golden;

import java.io.BufferedWriter;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HexFormat;
import java.util.SplittableRandom;

public final class JLangMain {
    private static final HexFormat HEX = HexFormat.of();
    private static final int N = 50_000;
    private final SplittableRandom rnd = new SplittableRandom(0x7A7A_0002L);
    private final BufferedWriter out;
    private long cases;

    private JLangMain(BufferedWriter out) {
        this.out = out;
    }

    public static void main(String[] args) throws Exception {
        Path dir = Path.of(args[0]);
        Files.createDirectories(dir);
        try (BufferedWriter out = Files.newBufferedWriter(dir.resolve("jlang.txt"), StandardCharsets.UTF_8)) {
            JLangMain main = new JLangMain(out);
            main.run();
            System.out.println("jlang.txt: " + main.cases + " cases");
        }
    }

    /** One call whose result is rendered as a golden token. */
    private interface Call {
        String run();
    }

    private static String i(int v) {
        return "i:" + v;
    }

    private static String l(long v) {
        return "l:" + v;
    }

    private static String f(float v) {
        return "f:" + HEX.toHexDigits(Float.floatToRawIntBits(v));
    }

    private static String d(double v) {
        return "d:" + HEX.toHexDigits(Double.doubleToRawLongBits(v));
    }

    private void rec(String op, Call call, String... args) throws IOException {
        String result;
        try {
            result = call.run();
        } catch (ArithmeticException | IllegalArgumentException e) {
            result = "s:" + HEX.formatHex(e.getClass().getSimpleName().getBytes(StandardCharsets.UTF_8));
        }
        StringBuilder line = new StringBuilder(op);
        for (String a : args) line.append(' ').append(a);
        line.append(" = ").append(result).append('\n');
        out.write(line.toString());
        cases++;
    }

    private static final float[] FS = {
        0f, -0f, 0.5f, -0.5f, 1.5f, -1.5f, 2.5f, -2.5f, 0.49999997f, -0.49999997f, 1f, -1f,
        Float.MIN_VALUE, -Float.MIN_VALUE, Float.MIN_NORMAL, Float.MAX_VALUE, -Float.MAX_VALUE,
        Float.POSITIVE_INFINITY, Float.NEGATIVE_INFINITY, Float.NaN,
        Float.intBitsToFloat(0xFFC0_0001), Float.intBitsToFloat(0x7F80_0001),
        8388607.5f, 8388608f, 8388609f, 2.1474836E9f, -2.1474836E9f, 1e30f, -1e30f, 90f, 360f,
    };

    private static final double[] DS = {
        0.0, -0.0, 0.5, -0.5, 1.5, -1.5, 2.5, -2.5, 0.49999999999999994, -0.49999999999999994,
        1.0, -1.0, Double.MIN_VALUE, -Double.MIN_VALUE, Double.MIN_NORMAL, Double.MAX_VALUE,
        -Double.MAX_VALUE, Double.POSITIVE_INFINITY, Double.NEGATIVE_INFINITY, Double.NaN,
        Double.longBitsToDouble(0xFFF8_0000_0000_0001L), Double.longBitsToDouble(0x7FF0_0000_0000_0001L),
        4503599627370495.5, 4503599627370496.0, 9.223372036854776E18, -9.223372036854776E18,
        1e300, -1e300, 90.0, 180.0, 360.0, Math.PI, -Math.PI / 2,
    };

    private static final int[] IS = {
        0, 1, -1, 2, -2, 3, 7, -7, 10, 46340, 46341, Integer.MAX_VALUE, Integer.MIN_VALUE,
        Integer.MAX_VALUE - 1, Integer.MIN_VALUE + 1,
    };

    private static final long[] LS = {
        0, 1, -1, 2, -2, 3, 7, -7, 10, 3037000499L, 3037000500L, Integer.MAX_VALUE,
        Integer.MIN_VALUE, (long) Integer.MAX_VALUE + 1, (long) Integer.MIN_VALUE - 1,
        Long.MAX_VALUE, Long.MIN_VALUE, Long.MAX_VALUE - 1, Long.MIN_VALUE + 1,
    };

    private float anyFloat() {
        return Float.intBitsToFloat(rnd.nextInt());
    }

    private double anyDouble() {
        return Double.longBitsToDouble(rnd.nextLong());
    }

    /** Values near halves and integers, where rounding is decided. */
    private double nearHalf() {
        double h = rnd.nextInt(-2_000_000, 2_000_001) * 0.5;
        return switch (rnd.nextInt(3)) {
            case 0 -> h;
            case 1 -> Math.nextUp(h);
            default -> Math.nextDown(h);
        };
    }

    private void floats(float a) throws IOException {
        rec("jl.round_f", () -> i(Math.round(a)), f(a));
        rec("jl.signum_f", () -> f(Math.signum(a)), f(a));
        rec("jl.next_up_f", () -> f(Math.nextUp(a)), f(a));
        rec("jl.next_down_f", () -> f(Math.nextDown(a)), f(a));
    }

    private void doubles(double a) throws IOException {
        rec("jl.round_d", () -> l(Math.round(a)), d(a));
        rec("jl.signum_d", () -> d(Math.signum(a)), d(a));
        rec("jl.to_radians", () -> d(Math.toRadians(a)), d(a));
        rec("jl.to_degrees", () -> d(Math.toDegrees(a)), d(a));
        rec("jl.next_up_d", () -> d(Math.nextUp(a)), d(a));
        rec("jl.next_down_d", () -> d(Math.nextDown(a)), d(a));
    }

    private void ints(int a, int b) throws IOException {
        rec("jl.abs_exact_i", () -> i(Math.absExact(a)), i(a));
        rec("jl.negate_exact_i", () -> i(Math.negateExact(a)), i(a));
        rec("jl.subtract_exact_i", () -> i(Math.subtractExact(a, b)), i(a), i(b));
        rec("jl.floor_div_exact_i", () -> i(Math.floorDivExact(a, b)), i(a), i(b));
    }

    private void longs(long a, long b) throws IOException {
        rec("jl.to_int_exact", () -> i(Math.toIntExact(a)), l(a));
        rec("jl.abs_exact_l", () -> l(Math.absExact(a)), l(a));
        rec("jl.negate_exact_l", () -> l(Math.negateExact(a)), l(a));
        rec("jl.subtract_exact_l", () -> l(Math.subtractExact(a, b)), l(a), l(b));
        rec("jl.floor_div_exact_l", () -> l(Math.floorDivExact(a, b)), l(a), l(b));
    }

    private void pows(long x, int n) throws IOException {
        int xi = (int) x;
        rec("jl.pow_exact_i", () -> i(Math.powExact(xi, n)), i(xi), i(n));
        rec("jl.pow_exact_l", () -> l(Math.powExact(x, n)), l(x), i(n));
    }

    private void clamps(long v, int lo, int hi, double dv, double dlo, double dhi) throws IOException {
        rec("jl.clamp_li", () -> i(Math.clamp(v, lo, hi)), l(v), i(lo), i(hi));
        long llo = lo, lhi = hi;
        rec("jl.clamp_l", () -> l(Math.clamp(v, llo, lhi)), l(v), l(llo), l(lhi));
        rec("jl.clamp_d", () -> d(Math.clamp(dv, dlo, dhi)), d(dv), d(dlo), d(dhi));
        float fv = (float) dv, flo = (float) dlo, fhi = (float) dhi;
        rec("jl.clamp_f", () -> f(Math.clamp(fv, flo, fhi)), f(fv), f(flo), f(fhi));
    }

    private void run() throws IOException {
        for (float a : FS) {
            floats(a);
            for (float b : FS) rec("jl.copy_sign_f", () -> f(Math.copySign(a, b)), f(a), f(b));
        }
        for (double a : DS) {
            doubles(a);
            for (double b : DS) rec("jl.copy_sign_d", () -> d(Math.copySign(a, b)), d(a), d(b));
        }
        for (int a : IS) for (int b : IS) ints(a, b);
        for (long a : LS) for (long b : LS) longs(a, b);
        for (long x : LS) for (int n : IS) pows(x, n);
        for (int x = -40; x <= 40; x++) for (int n = -2; n <= 70; n++) pows(x, n);
        for (int lo : IS) for (int hi : IS) for (long v : LS) {
            clamps(v, lo, hi, 0.5, 0.0, 1.0);
        }
        for (double lo : DS) for (double hi : DS) for (double v : new double[] {0.0, -0.0, 0.5, -1.0, Double.NaN, 1e300}) {
            clamps(0, 0, 1, v, lo, hi);
        }
        for (int k = 0; k < N; k++) {
            floats(anyFloat());
            floats((float) nearHalf());
            doubles(anyDouble());
            doubles(nearHalf());
            float ca = anyFloat(), cb = anyFloat();
            rec("jl.copy_sign_f", () -> f(Math.copySign(ca, cb)), f(ca), f(cb));
            ints(rnd.nextInt(), rnd.nextInt());
            ints(rnd.nextInt(-1000, 1000), rnd.nextInt(-50, 50));
            longs(rnd.nextLong(), rnd.nextLong());
            longs(rnd.nextLong(-100_000, 100_000), rnd.nextLong(-100, 100));
            pows(rnd.nextLong(-3_000_000_000L, 3_000_000_000L), rnd.nextInt(-1, 6));
            int lo = rnd.nextInt(), hi = rnd.nextInt();
            clamps(rnd.nextLong(), lo, hi, anyDouble(), rnd.nextDouble(-10, 10), rnd.nextDouble(-10, 10));
        }
    }
}

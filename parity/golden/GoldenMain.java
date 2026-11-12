/*
 * CeriumMC golden-case recorder. Copyright StepByte Studios, GPL-2.0-only.
 *
 * CI compiles this against the official vanilla server jar and runs it. It calls vanilla
 * code with many inputs and writes each call and its result, so the Rust tests can replay
 * them. The output is derived from Mojang's code: it stays in the CI cache and is never
 * committed or published. The file format is documented in tools/cerium-golden.
 */
package golden;

import java.io.BufferedWriter;
import java.io.IOException;
import java.io.UncheckedIOException;
import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HexFormat;
import java.util.List;
import java.util.SplittableRandom;
import java.util.UUID;
import net.minecraft.util.Mth;
import net.minecraft.util.RandomSource;
import net.minecraft.world.level.levelgen.LegacyRandomSource;
import net.minecraft.world.level.levelgen.PositionalRandomFactory;
import net.minecraft.world.level.levelgen.RandomSupport;
import net.minecraft.world.level.levelgen.SingleThreadedRandomSource;
import net.minecraft.world.level.levelgen.ThreadSafeLegacyRandomSource;
import net.minecraft.world.level.levelgen.WorldgenRandom;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;

public final class GoldenMain {
    public static void main(String[] args) throws Exception {
        Path dir = Path.of(args[0]);
        Files.createDirectories(dir);
        try (Recorder out = new Recorder(dir.resolve("mth.txt"))) {
            new MthCases(out).run();
        }
        try (Recorder out = new Recorder(dir.resolve("random.txt"))) {
            new RandomCases(out).run();
        }
        // Vanilla's static initializers may start non-daemon threads.
        System.exit(0);
    }
}

/** Builds one golden line at a time: {@code op args = results}. */
final class Recorder implements AutoCloseable {
    private static final HexFormat HEX = HexFormat.of();
    private final Path path;
    private final BufferedWriter writer;
    private final StringBuilder line = new StringBuilder();
    private long written;

    Recorder(Path path) throws IOException {
        this.path = path;
        this.writer = Files.newBufferedWriter(path, StandardCharsets.UTF_8);
    }

    Recorder op(String name) {
        line.setLength(0);
        line.append(name);
        return this;
    }

    Recorder i(int v) {
        line.append(" i:").append(v);
        return this;
    }

    Recorder l(long v) {
        line.append(" l:").append(v);
        return this;
    }

    Recorder b(byte v) {
        line.append(" b:").append(v);
        return this;
    }

    Recorder z(boolean v) {
        line.append(" z:").append(v ? '1' : '0');
        return this;
    }

    Recorder f(float v) {
        line.append(" f:").append(HEX.toHexDigits(Float.floatToRawIntBits(v)));
        return this;
    }

    Recorder d(double v) {
        line.append(" d:").append(HEX.toHexDigits(Double.doubleToRawLongBits(v)));
        return this;
    }

    Recorder s(String v) {
        line.append(" s:").append(HEX.formatHex(v.getBytes(StandardCharsets.UTF_8)));
        return this;
    }

    Recorder eq() {
        line.append(" =");
        return this;
    }

    void end() {
        try {
            writer.write(line.toString());
            writer.write('\n');
            written++;
        } catch (IOException e) {
            throw new UncheckedIOException(e);
        }
    }

    @Override
    public void close() throws IOException {
        writer.close();
        System.out.println(path.getFileName() + ": " + written + " cases");
    }
}

/** Random inputs biased toward edge cases. */
class Inputs {
    static final float[] SPECIAL_FLOATS = {
        0f, -0f, 1f, -1f, 0.5f, -0.5f, 2f, 0.25f, 0.75f, 0.99999994f, 1.0000001f,
        Float.MIN_VALUE, -Float.MIN_VALUE, Float.MIN_NORMAL, Float.MAX_VALUE, -Float.MAX_VALUE,
        Float.POSITIVE_INFINITY, Float.NEGATIVE_INFINITY, Float.NaN,
        45f, 90f, 180f, -180f, 270f, 360f, -360f, 720f, 179.99998f, -180.00002f,
        1e-5f, -1e-5f, 3.1415927f, -3.1415927f, 6.2831855f, 1e9f, -1e9f, 16777216f, 16777217f,
    };
    static final double[] SPECIAL_DOUBLES = {
        0.0, -0.0, 1.0, -1.0, 0.5, -0.5, 2.0, 0.25, 0.75, 0.9999999999999999, 1.0000000000000002,
        Double.MIN_VALUE, -Double.MIN_VALUE, Double.MIN_NORMAL, Double.MAX_VALUE, -Double.MAX_VALUE,
        Double.POSITIVE_INFINITY, Double.NEGATIVE_INFINITY, Double.NaN,
        45.0, 90.0, 180.0, -180.0, 270.0, 360.0, -360.0, 720.0, Math.PI, -Math.PI, 2 * Math.PI,
        1e-5, -1e-5, 1e9, -1e9, 2147483647.0, 2147483648.0, -2147483649.0, 9.2233720368547758E18,
        4503599627370496.0, 9007199254740993.0, 30000000.0, -30000000.5,
    };
    static final int[] SPECIAL_INTS = {
        0, 1, -1, 2, -2, 3, -3, 7, 8, 9, 15, 16, 17, 31, 32, 255, 256, 360, -360, 180, -180,
        65535, 65536, 1 << 30, (1 << 30) + 1, 30_000_000, -30_000_000, 29_999_984,
        Integer.MAX_VALUE, Integer.MIN_VALUE, Integer.MAX_VALUE - 1, Integer.MIN_VALUE + 1,
    };
    static final long[] SPECIAL_LONGS = {
        0L, 1L, -1L, 2L, 42L, 360L, -360L, 1L << 32, 1L << 40, (1L << 48) - 1, 1L << 62,
        Long.MAX_VALUE, Long.MIN_VALUE, Long.MAX_VALUE - 1, Long.MIN_VALUE + 1,
        Integer.MAX_VALUE, Integer.MIN_VALUE, 0x5DEECE66DL, -4658895280553007687L,
    };

    final SplittableRandom rnd;

    Inputs(long seed) {
        this.rnd = new SplittableRandom(seed);
    }

    float f() {
        int kind = rnd.nextInt(10);
        if (kind == 0) {
            return SPECIAL_FLOATS[rnd.nextInt(SPECIAL_FLOATS.length)];
        } else if (kind <= 2) {
            return Float.intBitsToFloat(rnd.nextInt());
        } else if (kind <= 5) {
            return (float) rnd.nextDouble(-10.0, 10.0);
        } else if (kind <= 7) {
            return rnd.nextFloat();
        }
        return (float) rnd.nextDouble(-100_000.0, 100_000.0);
    }

    double d() {
        int kind = rnd.nextInt(10);
        if (kind == 0) {
            return SPECIAL_DOUBLES[rnd.nextInt(SPECIAL_DOUBLES.length)];
        } else if (kind <= 2) {
            return Double.longBitsToDouble(rnd.nextLong());
        } else if (kind <= 5) {
            return rnd.nextDouble(-10.0, 10.0);
        } else if (kind <= 7) {
            return rnd.nextDouble();
        }
        return rnd.nextDouble(-100_000_000.0, 100_000_000.0);
    }

    int i() {
        int kind = rnd.nextInt(10);
        if (kind == 0) {
            return SPECIAL_INTS[rnd.nextInt(SPECIAL_INTS.length)];
        } else if (kind <= 3) {
            return rnd.nextInt();
        } else if (kind <= 6) {
            return rnd.nextInt(-1000, 1001);
        }
        return rnd.nextInt(-30_000_000, 30_000_001);
    }

    long l() {
        int kind = rnd.nextInt(10);
        if (kind == 0) {
            return SPECIAL_LONGS[rnd.nextInt(SPECIAL_LONGS.length)];
        } else if (kind <= 5) {
            return rnd.nextLong();
        } else if (kind <= 7) {
            return rnd.nextLong(-100_000, 100_001);
        }
        return i();
    }

    /** A float in a moderate range: no infinities, NaNs or huge magnitudes. */
    float moderateF() {
        return (float) rnd.nextDouble(-1000.0, 1000.0);
    }

    /** A positive int bound for {@code nextInt(bound)}, biased toward edge cases. */
    int bound() {
        return switch (rnd.nextInt(10)) {
            case 0 -> 1 << rnd.nextInt(31);
            case 1 -> Integer.MAX_VALUE;
            case 2 -> 1_431_655_765 + rnd.nextInt(-3, 4);
            case 3, 4 -> rnd.nextInt(1, 101);
            case 5 -> rnd.nextInt(1, Integer.MAX_VALUE);
            default -> rnd.nextInt(1, 100_001);
        };
    }

    /** A block coordinate: usually in the world border, sometimes any int. */
    int coord() {
        return switch (rnd.nextInt(10)) {
            case 0 -> rnd.nextInt();
            case 1 -> SPECIAL_INTS[rnd.nextInt(SPECIAL_INTS.length)];
            case 2, 3 -> rnd.nextInt(-64, 320);
            default -> rnd.nextInt(-30_000_000, 30_000_001);
        };
    }
}

/** Golden cases for {@link Mth}. */
final class MthCases extends Inputs {
    private static final int N = 4000;
    private final Recorder r;
    private long skipped;

    MthCases(Recorder r) {
        super(0xCE41_0002L);
        this.r = r;
    }

    /** Runs {@code body} {@code count} times; calls that throw in vanilla are not recorded. */
    private void each(int count, Runnable body) {
        for (int n = 0; n < count; n++) {
            try {
                body.run();
            } catch (RuntimeException e) {
                skipped++;
            }
        }
    }

    void run() throws ReflectiveOperationException {
        tables();
        r.op("const").s("PI").eq().f(Mth.PI).end();
        r.op("const").s("HALF_PI").eq().f(Mth.HALF_PI).end();
        r.op("const").s("TWO_PI").eq().f(Mth.TWO_PI).end();
        r.op("const").s("DEG_TO_RAD").eq().f(Mth.DEG_TO_RAD).end();
        r.op("const").s("RAD_TO_DEG").eq().f(Mth.RAD_TO_DEG).end();
        r.op("const").s("SQRT_OF_TWO").eq().f(Mth.SQRT_OF_TWO).end();

        each(N, () -> { double x = d(); r.op("sin").d(x).eq().f(Mth.sin(x)).end(); });
        each(N, () -> { double x = d(); r.op("cos").d(x).eq().f(Mth.cos(x)).end(); });
        each(N, () -> { float x = f(); r.op("sqrt").f(x).eq().f(Mth.sqrt(x)).end(); });
        each(N, () -> { float x = f(); r.op("floor_f32").f(x).eq().i(Mth.floor(x)).end(); });
        each(N, () -> { double x = d(); r.op("floor_f64").d(x).eq().i(Mth.floor(x)).end(); });
        each(N, () -> { double x = d(); r.op("lfloor").d(x).eq().l(Mth.lfloor(x)).end(); });
        each(N, () -> { float x = f(); r.op("abs_f32").f(x).eq().f(Mth.abs(x)).end(); });
        each(N, () -> { int x = i(); r.op("abs_i32").i(x).eq().i(Mth.abs(x)).end(); });
        each(N, () -> { float x = f(); r.op("ceil_f32").f(x).eq().i(Mth.ceil(x)).end(); });
        each(N, () -> { double x = d(); r.op("ceil_f64").d(x).eq().i(Mth.ceil(x)).end(); });
        each(N, () -> { double x = d(); r.op("ceil_long").d(x).eq().l(Mth.ceilLong(x)).end(); });
        each(N, () -> { int a = i(), b = i(), c = i();
            r.op("clamp_i32").i(a).i(b).i(c).eq().i(Mth.clamp(a, b, c)).end(); });
        each(N, () -> { long a = l(), b = l(), c = l();
            r.op("clamp_i64").l(a).l(b).l(c).eq().l(Mth.clamp(a, b, c)).end(); });
        each(N, () -> { float a = f(), b = f(), c = f();
            r.op("clamp_f32").f(a).f(b).f(c).eq().f(Mth.clamp(a, b, c)).end(); });
        each(N, () -> { double a = d(), b = d(), c = d();
            r.op("clamp_f64").d(a).d(b).d(c).eq().d(Mth.clamp(a, b, c)).end(); });
        each(N, () -> { double a = d(), b = d(), c = d();
            r.op("clamped_lerp_f64").d(a).d(b).d(c).eq().d(Mth.clampedLerp(a, b, c)).end(); });
        each(N, () -> { float a = f(), b = f(), c = f();
            r.op("clamped_lerp_f32").f(a).f(b).f(c).eq().f(Mth.clampedLerp(a, b, c)).end(); });
        each(N, () -> { int a = i(), b = i();
            r.op("abs_max_i32").i(a).i(b).eq().i(Mth.absMax(a, b)).end(); });
        each(N, () -> { float a = f(), b = f();
            r.op("abs_max_f32").f(a).f(b).eq().f(Mth.absMax(a, b)).end(); });
        each(N, () -> { double a = d(), b = d();
            r.op("abs_max_f64").d(a).d(b).eq().d(Mth.absMax(a, b)).end(); });
        each(N, () -> { int a = i(), b = i(), c = i(), e = i();
            r.op("chessboard_distance").i(a).i(b).i(c).i(e).eq()
                .i(Mth.chessboardDistance(a, b, c, e)).end(); });
        each(N, () -> { int a = i(), b = i();
            r.op("floor_div").i(a).i(b).eq().i(Mth.floorDiv(a, b)).end(); });
        each(N, () -> { float a = f(), b = rnd.nextBoolean() ? a + (float) rnd.nextDouble(-2e-5, 2e-5) : f();
            r.op("equal_f32").f(a).f(b).eq().z(Mth.equal(a, b)).end(); });
        each(N, () -> { double a = d(), b = rnd.nextBoolean() ? a + rnd.nextDouble(-2e-5, 2e-5) : d();
            r.op("equal_f64").d(a).d(b).eq().z(Mth.equal(a, b)).end(); });
        each(N, () -> { int a = i(), b = i();
            r.op("positive_modulo_i32").i(a).i(b).eq().i(Mth.positiveModulo(a, b)).end(); });
        each(N, () -> { float a = f(), b = f();
            r.op("positive_modulo_f32").f(a).f(b).eq().f(Mth.positiveModulo(a, b)).end(); });
        each(N, () -> { double a = d(), b = d();
            r.op("positive_modulo_f64").d(a).d(b).eq().d(Mth.positiveModulo(a, b)).end(); });
        each(N, () -> { int a = i(), b = rnd.nextBoolean() ? i() : rnd.nextInt(1, 20);
            r.op("is_multiple_of").i(a).i(b).eq().z(Mth.isMultipleOf(a, b)).end(); });
        each(N, () -> { float x = f(); r.op("pack_degrees").f(x).eq().b(Mth.packDegrees(x)).end(); });
        for (int v = Byte.MIN_VALUE; v <= Byte.MAX_VALUE; v++) {
            byte x = (byte) v;
            r.op("unpack_degrees").b(x).eq().f(Mth.unpackDegrees(x)).end();
        }
        each(N, () -> { int x = i(); r.op("wrap_degrees_i32").i(x).eq().i(Mth.wrapDegrees(x)).end(); });
        each(N, () -> { long x = l(); r.op("wrap_degrees_i64").l(x).eq().f(Mth.wrapDegrees(x)).end(); });
        each(N, () -> { float x = f(); r.op("wrap_degrees_f32").f(x).eq().f(Mth.wrapDegrees(x)).end(); });
        each(N, () -> { double x = d(); r.op("wrap_degrees_f64").d(x).eq().d(Mth.wrapDegrees(x)).end(); });
        each(N, () -> { float x = f(); r.op("wrap_degrees_90").f(x).eq().f(Mth.wrapDegrees90(x)).end(); });
        each(N, () -> { float a = f(), b = f();
            r.op("degrees_difference").f(a).f(b).eq().f(Mth.degreesDifference(a, b)).end(); });
        each(N, () -> { float a = f(), b = f();
            r.op("degrees_difference_abs").f(a).f(b).eq().f(Mth.degreesDifferenceAbs(a, b)).end(); });
        each(N, () -> { float a = f(), b = f(), c = f();
            r.op("rotate_if_necessary").f(a).f(b).f(c).eq().f(Mth.rotateIfNecessary(a, b, c)).end(); });
        each(N, () -> { float a = f(), b = f(), c = f();
            r.op("approach").f(a).f(b).f(c).eq().f(Mth.approach(a, b, c)).end(); });
        each(N, () -> { float a = f(), b = f(), c = f();
            r.op("approach_degrees").f(a).f(b).f(c).eq().f(Mth.approachDegrees(a, b, c)).end(); });
        each(N, () -> { int x = i(); r.op("smallest_encompassing_power_of_two").i(x).eq()
            .i(Mth.smallestEncompassingPowerOfTwo(x)).end(); });
        each(N, () -> { int x = i(); r.op("smallest_square_side").i(x).eq().i(Mth.smallestSquareSide(x)).end(); });
        each(N, () -> { int x = i(); r.op("is_power_of_two_i32").i(x).eq().z(Mth.isPowerOfTwo(x)).end(); });
        each(N, () -> { long x = rnd.nextBoolean() ? l() : 1L << rnd.nextInt(64);
            r.op("is_power_of_two_i64").l(x).eq().z(Mth.isPowerOfTwo(x)).end(); });
        each(N, () -> { int x = i(); r.op("ceillog2").i(x).eq().i(Mth.ceillog2(x)).end(); });
        each(N, () -> { int x = i(); r.op("log2").i(x).eq().i(Mth.log2(x)).end(); });
        each(N, () -> { float x = f(); r.op("frac_f32").f(x).eq().f(Mth.frac(x)).end(); });
        each(N, () -> { double x = d(); r.op("frac_f64").d(x).eq().d(Mth.frac(x)).end(); });
        each(N * 4, () -> { int x = coord(), y = coord(), z = coord();
            r.op("get_seed").i(x).i(y).i(z).eq().l(Mth.getSeed(x, y, z)).end(); });
        each(N, () -> { double a = d(), b = d(), c = d();
            r.op("inverse_lerp_f64").d(a).d(b).d(c).eq().d(Mth.inverseLerp(a, b, c)).end(); });
        each(N, () -> { float a = f(), b = f(), c = f();
            r.op("inverse_lerp_f32").f(a).f(b).f(c).eq().f(Mth.inverseLerp(a, b, c)).end(); });
        each(N * 4, () -> { double y = d(), x = d();
            r.op("atan2").d(y).d(x).eq().d(Mth.atan2(y, x)).end(); });
        each(N, () -> { float x = f(); r.op("inv_sqrt_f32").f(x).eq().f(Mth.invSqrt(x)).end(); });
        each(N, () -> { double x = d(); r.op("inv_sqrt_f64").d(x).eq().d(Mth.invSqrt(x)).end(); });
        each(N, () -> { double x = d(); r.op("fast_inv_sqrt").d(x).eq().d(Mth.fastInvSqrt(x)).end(); });
        each(N, () -> { float x = f(); r.op("fast_inv_cube_root").f(x).eq().f(Mth.fastInvCubeRoot(x)).end(); });
        each(N, () -> {
            float h = rnd.nextInt(8) == 0 ? f() : rnd.nextFloat();
            float s = rnd.nextInt(8) == 0 ? f() : rnd.nextFloat();
            float v = rnd.nextInt(8) == 0 ? f() : rnd.nextFloat();
            int a = i();
            r.op("hsv_to_argb").f(h).f(s).f(v).i(a).eq().i(Mth.hsvToArgb(h, s, v, a)).end();
        });
        each(N, () -> { int x = i(); r.op("murmur_hash3_mixer").i(x).eq().i(Mth.murmurHash3Mixer(x)).end(); });
        each(N, () -> {
            int from = rnd.nextInt(-1000, 1001);
            int to = from + rnd.nextInt(0, 2001);
            int threshold = rnd.nextInt(from - 10, to + 11);
            int result = Mth.binarySearch(from, to, x -> x >= threshold);
            r.op("binary_search").i(from).i(to).i(threshold).eq().i(result).end();
        });
        each(N, () -> { float a = f(); int p = i(), q = i();
            r.op("lerp_int").f(a).i(p).i(q).eq().i(Mth.lerpInt(a, p, q)).end(); });
        each(N, () -> { float a = f(); int p = i(), q = i();
            r.op("lerp_discrete").f(a).i(p).i(q).eq().i(Mth.lerpDiscrete(a, p, q)).end(); });
        each(N, () -> { float a = f(), p = f(), q = f();
            r.op("lerp_f32").f(a).f(p).f(q).eq().f(Mth.lerp(a, p, q)).end(); });
        each(N, () -> { double a = d(), p = d(), q = d();
            r.op("lerp_f64").d(a).d(p).d(q).eq().d(Mth.lerp(a, p, q)).end(); });
        each(N, () -> {
            float[] v = new float[6];
            for (int k = 0; k < v.length; k++) v[k] = f();
            r.op("lerp2_f32");
            for (float x : v) r.f(x);
            r.eq().f(Mth.lerp2(v[0], v[1], v[2], v[3], v[4], v[5])).end();
        });
        each(N, () -> {
            double[] v = new double[6];
            for (int k = 0; k < v.length; k++) v[k] = d();
            r.op("lerp2_f64");
            for (double x : v) r.d(x);
            r.eq().d(Mth.lerp2(v[0], v[1], v[2], v[3], v[4], v[5])).end();
        });
        each(N, () -> {
            float[] v = new float[11];
            for (int k = 0; k < v.length; k++) v[k] = f();
            r.op("lerp3_f32");
            for (float x : v) r.f(x);
            r.eq().f(Mth.lerp3(v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7], v[8], v[9], v[10])).end();
        });
        each(N, () -> {
            double[] v = new double[11];
            for (int k = 0; k < v.length; k++) v[k] = d();
            r.op("lerp3_f64");
            for (double x : v) r.d(x);
            r.eq().d(Mth.lerp3(v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7], v[8], v[9], v[10])).end();
        });
        each(N, () -> { float a = f(), p0 = f(), p1 = f(), p2 = f(), p3 = f();
            r.op("catmullrom").f(a).f(p0).f(p1).f(p2).f(p3).eq().f(Mth.catmullrom(a, p0, p1, p2, p3)).end(); });
        each(N, () -> { float x = f(); r.op("smoothstep").f(x).eq().f(Mth.smoothstep(x)).end(); });
        each(N, () -> { float x = f(); r.op("smoothstep_derivative").f(x).eq().f(Mth.smoothstepDerivative(x)).end(); });
        each(N, () -> { double x = d(); r.op("sign").d(x).eq().i(Mth.sign(x)).end(); });
        each(N, () -> { float a = f(), p = f(), q = f();
            r.op("rot_lerp_f32").f(a).f(p).f(q).eq().f(Mth.rotLerp(a, p, q)).end(); });
        each(N, () -> { double a = d(), p = d(), q = d();
            r.op("rot_lerp_f64").d(a).d(p).d(q).eq().d(Mth.rotLerp(a, p, q)).end(); });
        // rotLerpRad loops until the difference is in [-PI, PI); keep it finite and moderate.
        each(N, () -> { float a = f(), p = moderateF(), q = moderateF();
            r.op("rot_lerp_rad").f(a).f(p).f(q).eq().f(Mth.rotLerpRad(a, p, q)).end(); });
        each(N, () -> { float a = f(), b = f();
            r.op("triangle_wave").f(a).f(b).eq().f(Mth.triangleWave(a, b)).end(); });
        each(N, () -> { float x = f(); r.op("square_f32").f(x).eq().f(Mth.square(x)).end(); });
        each(N, () -> { float x = f(); r.op("cube").f(x).eq().f(Mth.cube(x)).end(); });
        each(N, () -> { double x = d(); r.op("square_f64").d(x).eq().d(Mth.square(x)).end(); });
        each(N, () -> { int x = i(); r.op("square_i32").i(x).eq().i(Mth.square(x)).end(); });
        each(N, () -> { long x = l(); r.op("square_i64").l(x).eq().l(Mth.square(x)).end(); });
        each(N, () -> { double v = d(), a = d(), b = d(), c = d(), e = d();
            r.op("clamped_map_f64").d(v).d(a).d(b).d(c).d(e).eq().d(Mth.clampedMap(v, a, b, c, e)).end(); });
        each(N, () -> { float v = f(), a = f(), b = f(), c = f(), e = f();
            r.op("clamped_map_f32").f(v).f(a).f(b).f(c).f(e).eq().f(Mth.clampedMap(v, a, b, c, e)).end(); });
        each(N, () -> { double v = d(), a = d(), b = d(), c = d(), e = d();
            r.op("map_f64").d(v).d(a).d(b).d(c).d(e).eq().d(Mth.map(v, a, b, c, e)).end(); });
        each(N, () -> { float v = f(), a = f(), b = f(), c = f(), e = f();
            r.op("map_f32").f(v).f(a).f(b).f(c).f(e).eq().f(Mth.map(v, a, b, c, e)).end(); });
        each(N, () -> { int a = i(), b = i();
            r.op("round_toward_i32").i(a).i(b).eq().i(Mth.roundToward(a, b)).end(); });
        each(N, () -> { long a = l(), b = l();
            r.op("round_toward_i64").l(a).l(b).eq().l(Mth.roundToward(a, b)).end(); });
        each(N, () -> { int a = i(), b = i();
            r.op("positive_ceil_div_i32").i(a).i(b).eq().i(Mth.positiveCeilDiv(a, b)).end(); });
        each(N, () -> { long a = l(), b = l();
            r.op("positive_ceil_div_i64").l(a).l(b).eq().l(Mth.positiveCeilDiv(a, b)).end(); });
        each(N, () -> { double x = d(), y = d();
            r.op("length_squared_2d_f64").d(x).d(y).eq().d(Mth.lengthSquared(x, y)).end(); });
        each(N, () -> { float x = f(), y = f();
            r.op("length_squared_2d_f32").f(x).f(y).eq().f(Mth.lengthSquared(x, y)).end(); });
        each(N, () -> { double x = d(), y = d();
            r.op("length_2d_f64").d(x).d(y).eq().d(Mth.length(x, y)).end(); });
        each(N, () -> { float x = f(), y = f();
            r.op("length_2d_f32").f(x).f(y).eq().f(Mth.length(x, y)).end(); });
        each(N, () -> { double x = d(), y = d(), z = d();
            r.op("length_squared_3d_f64").d(x).d(y).d(z).eq().d(Mth.lengthSquared(x, y, z)).end(); });
        each(N, () -> { double x = d(), y = d(), z = d();
            r.op("length_3d_f64").d(x).d(y).d(z).eq().d(Mth.length(x, y, z)).end(); });
        each(N, () -> { float x = f(), y = f(), z = f();
            r.op("length_3d_f32").f(x).f(y).f(z).eq().f(Mth.length(x, y, z)).end(); });
        each(N, () -> { float x = f(), y = f(), z = f();
            r.op("length_squared_3d_f32").f(x).f(y).f(z).eq().f(Mth.lengthSquared(x, y, z)).end(); });
        each(N, () -> { double v = d(); int q = i();
            r.op("quantize").d(v).i(q).eq().i(Mth.quantize(v, q)).end(); });
        each(N, () -> {
            int lower = rnd.nextInt(-50, 51);
            int upper = lower + rnd.nextInt(0, 61);
            int origin = rnd.nextInt(-70, 71);
            int step = rnd.nextInt(-1, 6);
            int[] values = Mth.outFromOrigin(origin, lower, upper, step).toArray();
            r.op("out_from_origin").i(origin).i(lower).i(upper).i(step).eq();
            for (int v : values) r.i(v);
            r.end();
        });
        System.out.println("mth: skipped " + skipped + " calls that throw in vanilla");
    }

    /** Vanilla's private lookup tables, read by reflection. */
    private void tables() throws ReflectiveOperationException {
        float[] sin = (float[]) field("SIN").get(null);
        for (int i = 0; i < sin.length; i++) r.op("sintab").i(i).eq().f(sin[i]).end();
        double[] asin = (double[]) field("ASIN_TAB").get(null);
        for (int i = 0; i < asin.length; i++) r.op("asintab").i(i).eq().d(asin[i]).end();
        double[] cos = (double[]) field("COS_TAB").get(null);
        for (int i = 0; i < cos.length; i++) r.op("costab").i(i).eq().d(cos[i]).end();
    }

    private static Field field(String name) throws NoSuchFieldException {
        Field field = Mth.class.getDeclaredField(name);
        field.setAccessible(true);
        return field;
    }
}

/** A random program over every vanilla generator type, replayed step by step in Rust. */
final class RandomCases extends Inputs {
    private static final int STEPS = 250_000;
    private static final String[] NAMES = {
        "", "a", "minecraft:overworld", "minecraft:ore_gold", "minecraft:aquifer",
        "minecraft:octave_-4", "minecraft:terrain", "héllo", "日本語",
        "😀emoji", "x".repeat(55), "y".repeat(56), "z".repeat(64), "w".repeat(200),
    };
    private final Recorder r;
    private final List<RandomSource> gens = new ArrayList<>();
    private final List<PositionalRandomFactory> facs = new ArrayList<>();

    RandomCases(Recorder r) {
        super(0x5EED_CE41L);
        this.r = r;
    }

    private int add(RandomSource gen) {
        gens.add(gen);
        return gens.size() - 1;
    }

    private void create(String kind, long seed) {
        RandomSource gen = switch (kind) {
            case "new.legacy" -> new LegacyRandomSource(seed);
            case "new.single" -> new SingleThreadedRandomSource(seed);
            case "new.threadsafe" -> new ThreadSafeLegacyRandomSource(seed);
            case "new.xoro" -> new XoroshiroRandomSource(seed);
            case "new.wg.legacy" -> new WorldgenRandom(new LegacyRandomSource(seed));
            case "new.wg.single" -> new WorldgenRandom(new SingleThreadedRandomSource(seed));
            case "new.wg.threadsafe" -> new WorldgenRandom(new ThreadSafeLegacyRandomSource(seed));
            case "new.wg.xoro" -> new WorldgenRandom(new XoroshiroRandomSource(seed));
            default -> throw new IllegalArgumentException(kind);
        };
        r.op(kind).i(gens.size()).l(seed).end();
        add(gen);
    }

    private static final String[] KINDS = {
        "new.legacy", "new.single", "new.threadsafe", "new.xoro",
        "new.wg.legacy", "new.wg.single", "new.wg.threadsafe", "new.wg.xoro",
    };

    String name() {
        if (rnd.nextBoolean()) {
            return NAMES[rnd.nextInt(NAMES.length)];
        }
        String alphabet = "abcdefghijklmnopqrstuvwxyz0123456789_:/.-é世";
        StringBuilder sb = new StringBuilder();
        int length = rnd.nextInt(0, 24);
        for (int k = 0; k < length; k++) sb.append(alphabet.charAt(rnd.nextInt(alphabet.length())));
        return sb.toString();
    }

    void run() {
        for (long seed : SPECIAL_LONGS) {
            for (String kind : KINDS) create(kind, seed);
        }
        for (int k = 0; k < 64; k++) create(KINDS[rnd.nextInt(KINDS.length)], l());
        for (int k = 0; k < 16; k++) {
            long lo = l(), hi = rnd.nextInt(4) == 0 ? 0L : l();
            if (k == 0) lo = 0L;
            r.op("new.xoro128").i(gens.size()).l(lo).l(hi).end();
            add(new XoroshiroRandomSource(lo, hi));
        }
        statics();
        for (int step = 0; step < STEPS; step++) step();
    }

    private void statics() {
        for (int k = 0; k < 2000; k++) {
            long z = l();
            r.op("mix").l(z).eq().l(RandomSupport.mixStafford13(z)).end();
            RandomSupport.Seed128bit mixed = RandomSupport.upgradeSeedTo128bit(z);
            r.op("seed128").l(z).eq().l(mixed.seedLo()).l(mixed.seedHi()).end();
            RandomSupport.Seed128bit unmixed = RandomSupport.upgradeSeedTo128bitUnmixed(z);
            r.op("seed128u").l(z).eq().l(unmixed.seedLo()).l(unmixed.seedHi()).end();
        }
        for (int k = 0; k < 500; k++) {
            String name = name();
            RandomSupport.Seed128bit seed = RandomSupport.seedFromHashOf(name);
            r.op("seedhash").s(name).eq().l(seed.seedLo()).l(seed.seedHi()).end();
        }
        for (int k = 0; k < 2000; k++) {
            double coord = rnd.nextInt(4) == 0 ? d() : rnd.nextDouble(-30_000_000.0, 30_000_000.0);
            r.op("wobble").d(coord).eq().d(Mth.wobble(coord)).end();
        }
    }

    private void step() {
        int id = rnd.nextInt(gens.size());
        RandomSource g = gens.get(id);
        int op = rnd.nextInt(100);
        if (op < 10) {
            r.op("ni").i(id).eq().i(g.nextInt()).end();
        } else if (op < 22) {
            int bound = bound();
            r.op("nib").i(id).i(bound).eq().i(g.nextInt(bound)).end();
        } else if (op < 26) {
            int min = rnd.nextBoolean() ? rnd.nextInt(-1000, 1001) : rnd.nextInt();
            long width = rnd.nextBoolean() ? rnd.nextInt(0, 100) : rnd.nextLong(0, Integer.MAX_VALUE);
            if ((long) min + width > Integer.MAX_VALUE) width = 10;
            if ((long) min + width > Integer.MAX_VALUE) min = 0;
            int max = (int) (min + width);
            r.op("nibi").i(id).i(min).i(max).eq().i(g.nextIntBetweenInclusive(min, max)).end();
        } else if (op < 29) {
            int origin = rnd.nextInt(-100_000, 100_001);
            int bound = origin + rnd.nextInt(1, 1_000_000);
            r.op("nir").i(id).i(origin).i(bound).eq().i(g.nextInt(origin, bound)).end();
        } else if (op < 37) {
            r.op("nl").i(id).eq().l(g.nextLong()).end();
        } else if (op < 41) {
            r.op("nb").i(id).eq().z(g.nextBoolean()).end();
        } else if (op < 48) {
            r.op("nf").i(id).eq().f(g.nextFloat()).end();
        } else if (op < 55) {
            r.op("nd").i(id).eq().d(g.nextDouble()).end();
        } else if (op < 63) {
            r.op("ng").i(id).eq().d(g.nextGaussian()).end();
        } else if (op < 65) {
            double mean = d(), spread = d();
            r.op("trid").i(id).d(mean).d(spread).eq().d(g.triangle(mean, spread)).end();
        } else if (op < 67) {
            float mean = f(), spread = f();
            r.op("trif").i(id).f(mean).f(spread).eq().f(g.triangle(mean, spread)).end();
        } else if (op < 69) {
            int rounds = rnd.nextInt(-2, 20);
            g.consumeCount(rounds);
            r.op("cc").i(id).i(rounds).end();
        } else if (op < 73) {
            long seed = l();
            g.setSeed(seed);
            r.op("ss").i(id).l(seed).end();
        } else if (op < 75) {
            int child = add(g.fork());
            r.op("fork").i(id).i(child).end();
        } else if (op < 77) {
            facs.add(g.forkPositional());
            r.op("fp").i(id).i(facs.size() - 1).end();
        } else if (op < 85) {
            worldgenOp(id, g);
        } else if (op < 91) {
            factoryOp();
        } else {
            mthOp(id, g);
        }
    }

    private void worldgenOp(int id, RandomSource g) {
        if (!(g instanceof WorldgenRandom w)) {
            r.op("ni").i(id).eq().i(g.nextInt()).end();
            return;
        }
        long seed = l();
        switch (rnd.nextInt(5)) {
            case 0 -> {
                int cx = coord() >> 4, cz = coord() >> 4;
                r.op("wg.dec").i(id).l(seed).i(cx).i(cz).eq().l(w.setDecorationSeed(seed, cx, cz)).end();
            }
            case 1 -> {
                int index = rnd.nextBoolean() ? rnd.nextInt(0, 64) : i();
                int step = rnd.nextBoolean() ? rnd.nextInt(0, 11) : i();
                w.setFeatureSeed(seed, index, step);
                r.op("wg.feat").i(id).l(seed).i(index).i(step).end();
            }
            case 2 -> {
                int cx = coord() >> 4, cz = coord() >> 4;
                w.setLargeFeatureSeed(seed, cx, cz);
                r.op("wg.large").i(id).l(seed).i(cx).i(cz).end();
            }
            case 3 -> {
                int x = coord() >> 4, z = coord() >> 4, blend = rnd.nextBoolean() ? rnd.nextInt(0, 100_000_000) : i();
                w.setLargeFeatureWithSalt(seed, x, z, blend);
                r.op("wg.salt").i(id).l(seed).i(x).i(z).i(blend).end();
            }
            default -> r.op("wg.count").i(id).eq().i(w.getCount()).end();
        }
    }

    private void factoryOp() {
        if (facs.isEmpty()) {
            return;
        }
        int fid = rnd.nextInt(facs.size());
        PositionalRandomFactory fac = facs.get(fid);
        switch (rnd.nextInt(4)) {
            case 0 -> {
                int x = coord(), y = coord(), z = coord();
                int child = add(fac.at(x, y, z));
                r.op("at").i(fid).i(child).i(x).i(y).i(z).end();
            }
            case 1 -> {
                String name = name();
                int child = add(fac.fromHashOf(name));
                r.op("hash").i(fid).i(child).s(name).end();
            }
            case 2 -> {
                long seed = l();
                int child = add(fac.fromSeed(seed));
                r.op("fseed").i(fid).i(child).l(seed).end();
            }
            default -> {
                StringBuilder sb = new StringBuilder();
                fac.parityConfigString(sb);
                r.op("fstr").i(fid).eq().s(sb.toString()).end();
            }
        }
    }

    private void mthOp(int id, RandomSource g) {
        switch (rnd.nextInt(8)) {
            case 0 -> {
                int min = rnd.nextInt(-1000, 1001), max = rnd.nextInt(-1000, 1001);
                r.op("mth.nextint").i(id).i(min).i(max).eq().i(Mth.nextInt(g, min, max)).end();
            }
            case 1 -> {
                float min = f(), max = f();
                r.op("mth.nextfloat").i(id).f(min).f(max).eq().f(Mth.nextFloat(g, min, max)).end();
            }
            case 2 -> {
                double min = d(), max = d();
                r.op("mth.nextdouble").i(id).d(min).d(max).eq().d(Mth.nextDouble(g, min, max)).end();
            }
            case 3 -> {
                int min = rnd.nextInt(-1000, 1001), max = min + rnd.nextInt(0, 1000);
                r.op("mth.rbi").i(id).i(min).i(max).eq().i(Mth.randomBetweenInclusive(g, min, max)).end();
            }
            case 4 -> {
                float min = f(), max = f();
                r.op("mth.rb").i(id).f(min).f(max).eq().f(Mth.randomBetween(g, min, max)).end();
            }
            case 5 -> {
                float mean = f(), dev = f();
                r.op("mth.normal").i(id).f(mean).f(dev).eq().f(Mth.normal(g, mean, dev)).end();
            }
            case 6 -> {
                UUID uuid = Mth.createInsecureUUID(g);
                r.op("mth.uuid").i(id).eq().l(uuid.getMostSignificantBits())
                    .l(uuid.getLeastSignificantBits()).end();
            }
            default -> {
                int x = coord(), z = coord();
                long seed = l(), salt = rnd.nextBoolean() ? 987234911L : l();
                int child = add(WorldgenRandom.seedSlimeChunk(x, z, seed, salt));
                r.op("slime").i(child).i(x).i(z).l(seed).l(salt).end();
            }
        }
    }
}

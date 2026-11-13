/*
 * CeriumMC org.joml.Math recorder. Copyright StepByte Studios, GPL-2.0-only.
 *
 * Records the results of the org.joml.Math methods vanilla calls (sin, cosFromSin, sqrt,
 * invsqrt, abs, max), using the JOML jar the server bundles and JOML's default settings, as
 * the server runs it. JOML is MIT-licensed; no Minecraft code is used.
 *
 * Usage: java -cp <classes>:<joml.jar> golden.JomlMain <output-dir>
 */
package golden;

import java.io.BufferedWriter;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HexFormat;
import java.util.SplittableRandom;

public final class JomlMain {
    private static final HexFormat HEX = HexFormat.of();
    private static final int N = 200_000;
    private final SplittableRandom rnd = new SplittableRandom(0x7A7A_0003L);
    private final BufferedWriter out;
    private long cases;

    private JomlMain(BufferedWriter out) {
        this.out = out;
    }

    public static void main(String[] args) throws Exception {
        Path dir = Path.of(args[0]);
        Files.createDirectories(dir);
        try (BufferedWriter out = Files.newBufferedWriter(dir.resolve("joml.txt"), StandardCharsets.UTF_8)) {
            JomlMain main = new JomlMain(out);
            main.run();
            System.out.println("joml.txt: " + main.cases + " cases");
        }
    }

    private static String f(float v) {
        return "f:" + HEX.toHexDigits(Float.floatToRawIntBits(v));
    }

    private void rec(String op, float result, float... args) throws IOException {
        StringBuilder line = new StringBuilder(op);
        for (float a : args) line.append(' ').append(f(a));
        line.append(" = ").append(f(result)).append('\n');
        out.write(line.toString());
        cases++;
    }

    private static final float[] SPECIAL = {
        0f, -0f, 0.5f, -0.5f, 1f, -1f, 2f, 1.5707964f, -1.5707964f, 3.1415927f, -3.1415927f,
        6.2831855f, -6.2831855f, 0.7853982f, 1e-30f, Float.MIN_VALUE, Float.MIN_NORMAL,
        Float.MAX_VALUE, -Float.MAX_VALUE, Float.POSITIVE_INFINITY, Float.NEGATIVE_INFINITY,
        Float.NaN, Float.intBitsToFloat(0xFFC0_0001), 1e10f, -1e10f, 100f, -100f,
    };

    private float anyFloat() {
        return Float.intBitsToFloat(rnd.nextInt());
    }

    private void unary(float a) throws IOException {
        rec("joml.sin", org.joml.Math.sin(a), a);
        rec("joml.sqrt", org.joml.Math.sqrt(a), a);
        rec("joml.invsqrt", org.joml.Math.invsqrt(a), a);
        rec("joml.abs", org.joml.Math.abs(a), a);
    }

    private void binary(float a, float b) throws IOException {
        rec("joml.max", org.joml.Math.max(a, b), a, b);
        rec("joml.cos_from_sin", org.joml.Math.cosFromSin(a, b), a, b);
    }

    private void run() throws IOException {
        for (float a : SPECIAL) {
            unary(a);
            for (float b : SPECIAL) binary(a, b);
            binary(org.joml.Math.sin(a), a);
        }
        for (int i = 0; i < N; i++) {
            float angle = (float) rnd.nextDouble(-20.0, 20.0);
            unary(angle);
            unary(anyFloat());
            binary(org.joml.Math.sin(angle), angle);
            binary(org.joml.Math.sin(angle / 2.0F), angle / 2.0F);
            binary(anyFloat(), anyFloat());
            binary((float) rnd.nextDouble(-1.0, 1.0), (float) rnd.nextDouble(-1e6, 1e6));
        }
    }
}

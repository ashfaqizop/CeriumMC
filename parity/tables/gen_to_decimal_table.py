"""Computes the table of Java's Double/Float.toString (jdk.internal.math.MathUtils.g).

For K_MIN <= k <= K_MAX, write 10^-k = beta * 2^r with 2^125 <= beta < 2^126 and let
g = floor(beta) + 1, split into g1 = g >> 63 and g0 = g & (2^63 - 1). The values are computed
here exactly with integers rather than copied. When the JDK's MathUtils.java is given, the
result is checked against its table entry by entry.

Copyright StepByte Studios, GPL-2.0-only.

Usage: python -I gen_to_decimal_table.py OUTPUT.rs [MathUtils.java]
"""
import re
import sys

K_MIN, K_MAX = -324, 292


def g_of(k: int) -> int:
    # 10^-k = num / den exactly.
    num, den = (10 ** -k, 1) if k <= 0 else (1, 10 ** k)
    # r is the unique integer with 2^125 <= (num / den) / 2^r < 2^126.
    r = num.bit_length() - den.bit_length() - 126
    while True:
        n, d = (num, den << r) if r >= 0 else (num << -r, den)
        if n < d << 125:
            r -= 1
        elif n >= d << 126:
            r += 1
        else:
            return n // d + 1


def hex64(value: int) -> str:
    digits = "%016X" % value
    return "0x" + "_".join(digits[i : i + 4] for i in range(0, 16, 4))


def main() -> None:
    output = sys.argv[1]
    table = []
    for k in range(K_MIN, K_MAX + 1):
        g = g_of(k)
        table.append((g >> 63, g & ((1 << 63) - 1)))
    if len(sys.argv) > 2:
        src = open(sys.argv[2], encoding="utf-8").read()
        body = src[src.index("private static final long[] g = {") :]
        jdk = [int(v.replace("_", ""), 16) for v in re.findall(r"0x([0-9A-F_]+)L", body)]
        pairs = list(zip(jdk[0::2], jdk[1::2]))
        assert pairs == table, "computed table differs from the JDK's"
        print("matches the JDK table")
    lines = [
        "//! The table of `jdk.internal.math.MathUtils.g` that Java's `Double.toString` and\n",
        "//! `Float.toString` use, computed exactly by `parity/tables/gen_to_decimal_table.py`.\n",
        "//! Do not edit by hand.\n\n",
        f"/// `(g1(k), g0(k))` for `k` from {K_MIN} to {K_MAX}.\n",
        "#[rustfmt::skip]\n",
        f"pub(super) static G: [(u64, u64); {len(table)}] = [\n",
    ]
    for k, (g1, g0) in zip(range(K_MIN, K_MAX + 1), table):
        lines.append(f"    ({hex64(g1)}, {hex64(g0)}), // {k}\n")
    lines.append("];\n")
    with open(output, "w", encoding="utf-8", newline="\n") as f:
        f.write("".join(lines))


if __name__ == "__main__":
    main()

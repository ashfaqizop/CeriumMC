"""Extracts the constant tables of an OpenJDK HotSpot x86_64 math stub into Rust source.

Copyright StepByte Studios, GPL-2.0-only. The extracted data comes from OpenJDK
(stubGenerator_x86_64_*.cpp at tag jdk-25.0.1-ga), Copyright Intel Corporation and others,
GPL-2.0-only. The generated file keeps that notice.

Usage: python -I extract_stub_tables.py STUB.cpp OUTPUT.rs
"""
import re
import sys

NOTICE = """//! Constant tables of OpenJDK's `{name}` (tag `jdk-25.0.1-ga`),
//! extracted verbatim by `parity/tables/extract_stub_tables.py`. Do not edit by hand.
//!
//! Original notice of the source file:
//!
{header}
"""


def main() -> None:
    source_path, output_path = sys.argv[1], sys.argv[2]
    src = open(source_path, encoding="utf-8").read()
    header = src[: src.index("*/") + 2]
    header_lines = "\n".join(
        ("//! " + line.strip().lstrip("/*").strip()).rstrip() for line in header.splitlines()
    )
    name = source_path.replace("\\", "/").split("/")[-1].replace("j25_", "stubGenerator_x86_64_")
    out = [NOTICE.format(name=name, header=header_lines)]
    for match in re.finditer(r"static const juint (_\w+)\[\]\s*=\s*\{(.*?)\};", src, re.S):
        table, body = match.group(1), match.group(2)
        values = re.findall(r"0x([0-9a-fA-F]+)UL", body)
        rust_name = table.lstrip("_").upper()
        out.append(f"#[rustfmt::skip]\npub(super) const {rust_name}: [u32; {len(values)}] = [\n")
        for i in range(0, len(values), 4):
            chunk = values[i : i + 4]
            out.append("    " + ", ".join("0x%s_%s" % (v.zfill(8)[:4].upper(), v.zfill(8)[4:].upper()) for v in chunk) + ",\n")
        out.append("];\n\n")
    with open(output_path, "w", encoding="utf-8", newline="\n") as f:
        f.write("".join(out).rstrip("\n") + "\n")


if __name__ == "__main__":
    main()

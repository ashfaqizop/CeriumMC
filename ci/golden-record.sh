#!/usr/bin/env bash
# Compiles the golden harness (parity/golden) against the vanilla server and records golden
# cases into .vanilla/golden. The server's bundler unpacked the real server jar and its
# libraries into .vanilla/run during the boot steps. Run from the repository root, after
# ci/vanilla.env is loaded. The output is Mojang-derived: it stays in the CI cache only.
set -euo pipefail

run=.vanilla/run
jar="$run/versions/${VANILLA_VERSION}/server-${VANILLA_VERSION}.jar"
test -f "$jar"

classpath="$jar"
while IFS= read -r lib; do
  classpath="$classpath:$lib"
done < <(find "$run/libraries" -name '*.jar' | sort)

mkdir -p .vanilla/golden-classes .vanilla/golden
javac --release "${VANILLA_JAVA}" -nowarn -encoding UTF-8 -cp "$classpath" \
  -d .vanilla/golden-classes parity/golden/*.java
java -Xmx2G -cp ".vanilla/golden-classes:$classpath" golden.GoldenMain .vanilla/golden
ls -la .vanilla/golden

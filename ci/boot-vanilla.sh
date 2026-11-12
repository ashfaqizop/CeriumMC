#!/usr/bin/env bash
# Boots the vanilla reference server once and records a boot report.
# Usage: ci/boot-vanilla.sh LABEL   (run from the repository root, after ci/vanilla.env is loaded)
set -euo pipefail

label="$1"
target/release/cerium-probe boot \
  --label "vanilla-${VANILLA_VERSION}-${label}" \
  --cwd .vanilla/run \
  --timeout-secs 600 \
  --settle-secs 10 \
  --stop-command stop \
  --stop-timeout-secs 180 \
  --expect-version "${VANILLA_VERSION}" \
  --expect-protocol "${VANILLA_PROTOCOL}" \
  --report "boot-vanilla-${label}.json" \
  -- java -Xms2G -Xmx2G -jar ../server.jar --nogui

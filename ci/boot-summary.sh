#!/usr/bin/env bash
# Renders boot reports (JSON written by `cerium-probe boot --report`) as a Markdown table.
# Usage: ci/boot-summary.sh REPORT.json...
set -euo pipefail

echo "### Boot benchmark"
echo
echo "| run | version | protocol | time to status | ping | RSS after settle | peak RSS | shutdown | exit |"
echo "|---|---|---|---|---|---|---|---|---|"
for report in "$@"; do
  [ -f "$report" ] || continue
  jq -r '"| \(.label) | \(.version_name) | \(.protocol) | \(.time_to_status_ms) ms | \(.ping_us) µs | \(.rss_kib) KiB | \(.peak_rss_kib) KiB | \(.shutdown_ms) ms | \(.exit_code) |"' "$report"
done

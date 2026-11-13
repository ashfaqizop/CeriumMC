#!/usr/bin/env bash
# Prints this machine's CPUID vendor string (GenuineIntel, AuthenticAMD, ...), or "unknown".
# HotSpot's x86_64 Math.log, log10 and pow depend on it (RCPPS), so every recording of
# java.lang.Math results stores it next to the data as cpu-vendor.txt.
set -euo pipefail

case "$(uname -s)" in
  Linux) vendor="$(grep -m1 '^vendor_id' /proc/cpuinfo | cut -d: -f2 || true)" ;;
  Darwin) vendor="$(sysctl -n machdep.cpu.vendor 2> /dev/null || true)" ;;
  MINGW* | MSYS* | CYGWIN*)
    vendor="$(powershell -NoProfile -Command '(Get-CimInstance Win32_Processor).Manufacturer')" ;;
  *) vendor="" ;;
esac
vendor="$(printf '%s' "$vendor" | tr -d ' \t\r\n')"
echo "${vendor:-unknown}"

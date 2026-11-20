#!/usr/bin/env bash
# Usage: benchmarks/imgui/run_imgui_bench.sh <path-to-dear-imgui-checkout>
# Example: git clone --depth 1 https://github.com/ocornut/imgui /tmp/imgui
set -euo pipefail
IMGUI_DIR="${1:?path to a Dear ImGui checkout}"
HERE="$(cd "$(dirname "$0")" && pwd)"
OUT="${TMPDIR:-/tmp}/rugi_imgui_bench"
# -O3 + LTO, like the `dist` profile the Rust benchmarks are built with.
c++ -O3 -flto -DNDEBUG -std=c++17 -I"$IMGUI_DIR" \
  "$HERE/imgui_bench.cpp" "$IMGUI_DIR/imgui.cpp" "$IMGUI_DIR/imgui_draw.cpp" \
  "$IMGUI_DIR/imgui_widgets.cpp" "$IMGUI_DIR/imgui_tables.cpp" -o "$OUT"
"$OUT"

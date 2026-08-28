#!/usr/bin/env bash
# derive-sources-prefix.sh <symbols.so> <crate-src-suffix>
#
# Print the SourcesOriginalPath that will actually match the coverage profile,
# or exit non-zero if none does.
#
# The cover task keys every LCOV line on a source path and then runs the
# equivalent of `Info.Has(SourcesOriginalPath)`. If nothing carries that prefix
# it FAILS the whole task -- "SourcesOriginalPath ... does not match any source
# file in the coverage profile" -- and the project renders lines_found: 0 while
# every CI step stays green. It is the most expensive failure mode here because
# nothing else reports it.
#
# READ THE LINE TABLE, NOT .debug_info. The SF: path is
# `DW_AT_comp_dir` (when present) + the line program's `include_directories[]`
# entry + `file_names[]`. It is NOT the compile unit's DW_AT_name -- that
# carries a codegen-unit suffix (`.../lib.rs/@/crate.hash-cgu.00`) and, on real
# SBF artifacts, differs from what actually ends up in the profile. Deriving
# from DW_AT_name looked right, produced `programs/`, and the server rejected
# it anyway.
#
# Two further traps this accounts for:
#   * comp_dir is FREQUENTLY ABSENT from these artifacts, so "comp_dir + known
#     suffix" silently degrades to a bare guess;
#   * include_directories mixes first-party entries (`programs/<crate>/src`),
#     bare relative ones (`src`, from dependencies) and absolute toolchain paths
#     (`/Users/runner/.../platform-tools/...`) in a single binary.
set -euo pipefail

so="${1:?usage: derive-sources-prefix.sh <symbols.so> <crate-src-suffix>}"
suffix="${2:?missing crate src suffix, e.g. programs/shielded-pool/src/}"

dd=""
command -v llvm-dwarfdump >/dev/null 2>&1 && dd=llvm-dwarfdump
[ -z "$dd" ] && command -v dwarfdump >/dev/null 2>&1 && dd=dwarfdump
# Debian's `llvm` package installs a VERSIONED binary and no unsuffixed alias, so
# `command -v llvm-dwarfdump` finds nothing on a runner that DID install llvm.
if [ -z "$dd" ]; then
  for c in /usr/lib/llvm-*/bin/llvm-dwarfdump /usr/bin/llvm-dwarfdump-*; do
    [ -x "$c" ] && dd="$c" && break
  done
fi
[ -n "$dd" ] || { echo "no llvm-dwarfdump available" >&2; exit 2; }

# comp_dir, if the artifact carries one at all, prefixes every relative entry.
comp="$("$dd" --debug-info "$so" 2>/dev/null \
  | grep -oE 'DW_AT_comp_dir[[:space:]]*\("[^"]*"\)' \
  | sed -E 's/.*\("([^"]*)"\)/\1/' \
  | grep -vE '\.cargo|/rustc/|toolchain|bpf-tools|platform-tools' | head -1 || true)"

# Every directory the line program can attribute a line to.
"$dd" --debug-line "$so" 2>/dev/null \
  | grep -oE 'include_directories\[[[:space:]]*[0-9]+\][[:space:]]*=[[:space:]]*"[^"]*"' \
  | sed -E 's/.*"([^"]*)"/\1/' \
  | while IFS= read -r dir; do
      case "$dir" in /*) printf '%s\n' "$dir" ;;
                      *) printf '%s\n' "${comp:+${comp%/}/}$dir" ;;
      esac
    done \
  | awk -v suffix="$suffix" '
      { i = index($0 "/", suffix)
        if (i > 0) print substr($0, 1, i + length(suffix) - 1) }
    ' \
  | sort | uniq -c | sort -rn | head -1 | awk '{print $2}' \
  | grep . || { echo "no line-table directory contains $suffix" >&2; exit 1; }

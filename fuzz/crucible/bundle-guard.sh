#!/usr/bin/env bash
# bundle-guard.sh — fail-closed checks for a crucible FuzzCorp bundle.
#
#   ./bundle-guard.sh <bundle-dir> [repo-root]      # verify (and stage missing sources)
#   CHECK_ONLY=1 ./bundle-guard.sh <bundle-dir>     # verify without staging anything
#
# Run this AFTER build-bundle.sh and BEFORE uploading. Every failure it catches is
# otherwise SILENT: the bundle validates, uploads, runs, and quietly produces
# nothing (or a fraction of what it should). None of them surface as an error, so
# the first sign is an empty dashboard days later.
#
#   GATE A  the harness actually has a fuzz test compiled in. Built without
#           `--features <feature>` the binary starts, selects no test, prints
#           "No fuzz test selected" and exits 0 -- a green campaign that fuzzed
#           nothing.
#   GATE B  every source path in the DWARF resolves to a real file in the bundle,
#           using the driver's OWN rule (fuzzcorp lib/coverage/lcov/lcov.go:245,
#           `Info.Replace`): a record whose key starts with SourcesOriginalPath is
#           rewritten into SourcesPathInBundle; a record that does NOT match keeps
#           its key verbatim and resolves relative to the BUNDLE ROOT. Programs
#           that span two source roots (programs/ + libraries/, program-libs/, ...)
#           lose the second one entirely unless it is staged at the bundle root.
#   GATE C  the manifest contains no absolute build-machine path. Those never match
#           on a CI-built bundle, and they leak the builder's username.
#   GATE D  the harness binary is linux/amd64. Workers are amd64; a mismatched
#           bundle validates and is then never picked up -- it fails as silence.
#           Non-static linkage is reported loudly but is not fatal, because a
#           glibc build MAY still run depending on the worker image.
#   GATE F  no COMMITTED harness source embeds a build-machine absolute path. The
#           generator emits a `SCOUT_GENERATION_PROVENANCE` constant recording the
#           absolute path of each of its inputs -- nothing reads it, and it carries
#           the operator's home directory and username into a file that ships to the
#           client. It comes back on every regeneration, so this has to be a gate
#           rather than a one-time cleanup.
#   GATE G  no COMMITTED harness source names the authoring toolchain -- its scripts,
#           its private docs, its commands. Same reason as GATE F: the generated
#           header re-appears on every regeneration, and none of those references
#           resolves to anything the client can open.
set -euo pipefail

BUNDLE="${1:?usage: bundle-guard.sh <bundle-dir> [repo-root]}"
REPO="${2:-}"
BUNDLE="$(cd "$BUNDLE" && pwd)"
[[ -n "$REPO" ]] && REPO="$(cd "$REPO" && pwd)"

HERE="$(cd "$(dirname "$0")" && pwd)"

python3 - "$BUNDLE" "$REPO" "${CHECK_ONLY:-0}" "$HERE" <<'PY'
import json, os, re, shutil, subprocess, sys

bundle, repo, check_only = sys.argv[1], sys.argv[2], sys.argv[3] == "1"
mf = os.path.join(bundle, "manifest.fc.json")
errors, warnings, notes = [], [], []

def strings(path):
    try:
        out = subprocess.run(["strings", "-a", path], capture_output=True,
                             text=True, errors="replace", timeout=1800).stdout
        if out.strip():
            return out
    except Exception:
        pass
    try:                                    # busybox / no binutils fallback
        data = open(path, "rb").read()
        return "\n".join(re.findall(rb"[\x20-\x7e]{4,}", data).__iter__().__str__()
                         for _ in [0])
    except Exception:
        return ""

if not os.path.exists(mf):
    print(f"ERROR: no manifest at {mf}", file=sys.stderr); sys.exit(1)
try:
    man = json.load(open(mf))
except json.JSONDecodeError as e:
    # A build-bundle.sh that interpolates an EMPTY shell var into its manifest
    # heredoc leaves a dangling comma or a blank key -- valid shell, invalid JSON.
    # The upload then fails with something far less obvious. Show the offending
    # region rather than a bare traceback.
    lines = open(mf, encoding="utf-8", errors="replace").read().split("\n")
    lo, hi = max(0, e.lineno - 4), min(len(lines), e.lineno + 3)
    print(f"ERROR: {mf} is not valid JSON: {e.msg} (line {e.lineno}, col {e.colno})",
          file=sys.stderr)
    print("       An empty shell variable in the manifest heredoc is the usual cause.",
          file=sys.stderr)
    for i in range(lo, hi):
        mark = ">>" if i + 1 == e.lineno else "  "
        print(f"  {mark} {i+1:4}| {lines[i]}", file=sys.stderr)
    sys.exit(1)

# ---------------------------------------------------------------- GATE C ----
raw = open(mf, encoding="utf-8").read()
tracked = subprocess.run(["git", "ls-files", "--error-unmatch", mf],
                         capture_output=True, cwd=os.path.dirname(mf) or ".").returncode == 0
for m in re.finditer(r'"([^"]*/(?:Users|home)/[^"]*)"', raw):
    lit = m.group(1)
    # An absolute SourcesOriginalPath is CORRECT when the DWARF genuinely carries the
    # build machine's path AND the manifest is regenerated next to the .so on that same
    # machine (loopscale does exactly this, deliberately). It is only a defect when the
    # manifest is a COMMITTED artifact: then it is frozen to one machine, matches nothing
    # anywhere else, and ships someone's username to the client.
    if tracked:
        errors.append(f"GATE C: COMMITTED manifest embeds an absolute build-machine path:\n"
                      f"        {lit[:110]}\n"
                      f"        Frozen to one machine: it cannot match a CI-built bundle (coverage\n"
                      f"        renders empty) and it leaks the builder's username to the client.")
    elif not os.path.exists(lit.rstrip("/")):
        errors.append(f"GATE C: manifest references an absolute path that does not exist here:\n"
                      f"        {lit[:110]}\n"
                      f"        The .so was built elsewhere, so this prefix matches nothing.")
    else:
        notes.append("GATE C: absolute SourcesOriginalPath, generated locally and not committed "
                     "(valid: it matches this machine's DWARF)")

# ---------------------------------------------------------------- GATE F ----
harness_dir = sys.argv[4] if len(sys.argv) > 4 else os.path.dirname(bundle)
tracked_src = subprocess.run(["git", "ls-files", "--", "src", "idls", "*.sh", "*.toml"],
                             capture_output=True, text=True, cwd=harness_dir)
ABS = re.compile(r'(?<![\w-])/(?:Users|home)/[A-Za-z0-9._-]+/')
for rel in tracked_src.stdout.split():
    full = os.path.join(harness_dir, rel)
    try:
        text = open(full, encoding="utf-8", errors="replace").read()
    except OSError:
        continue
    for m in ABS.finditer(text):
        line = text.count("\n", 0, m.start()) + 1
        snippet = text[m.start():m.start() + 90].split("\n")[0]
        # A path inside a comment illustrating someone ELSE's machine (a CI runner) is
        # documentation, not a leak; a real one appears in code or data.
        if "runner" in snippet:
            continue
        errors.append(f"GATE F: committed harness source embeds a build-machine path:\n"
                      f"        {rel}:{line}: {snippet}\n"
                      f"        This ships the builder's home directory and username to the\n"
                      f"        client. If it is SCOUT_GENERATION_PROVENANCE, delete the constant:\n"
                      f"        nothing reads it and regeneration re-emits it every time.")
        break

# ---------------------------------------------------------------- GATE G ----
# Same fail-closed reasoning as GATE F, for a different leak. The generator stamps a
# `GENERATED by <tool> <script>.py` header, and hand-written comments accumulate citations
# to the authoring toolchain's own private docs and commands. None of it resolves to
# anything the client can open, so it reads as a dangling reference in a file they now own.
# The header comes back on every regeneration, so it has to be a gate, not a cleanup.
TOOLING = re.compile(r'crucible-scout|references/[a-z-]+\.md|(?<![\w.])[a-z_]+\.py\b|`scout [a-z]+`')
for rel in tracked_src.stdout.split():
    # This file has to spell out what it forbids, so it cannot be subject to its own gate.
    if os.path.basename(rel) == "bundle-guard.sh":
        continue
    full = os.path.join(harness_dir, rel)
    try:
        text = open(full, encoding="utf-8", errors="replace").read()
    except OSError:
        continue
    m = TOOLING.search(text)
    if m:
        line = text.count("\n", 0, m.start()) + 1
        errors.append(f"GATE G: committed harness source names the authoring toolchain:\n"
                      f"        {rel}:{line}: {text[m.start():m.start() + 90].splitlines()[0]}\n"
                      f"        The client cannot resolve it. Describe the thing, not the tool.")

for lin in man.get("Lineages", []):
    for conf in lin.get("Confs", []):
        p = conf.get("Driver", {}).get("Params", {})
        name = lin.get("Name", "?")
        binp  = p.get("BinaryPathInBundle")
        symp  = p.get("SymbolsPathInBundle")
        srcs  = p.get("SourcesPathInBundle")
        orig  = p.get("SourcesOriginalPath")

        # ------------------------------------------------------- GATE A/D ---
        if not binp:
            errors.append(f"GATE A [{name}]: no BinaryPathInBundle"); continue
        bpath = os.path.join(bundle, binp)
        if not os.path.exists(bpath):
            errors.append(f"GATE A [{name}]: binary missing: {binp}"); continue

        feature = os.path.basename(binp)
        if feature not in strings(bpath):
            errors.append(f"GATE A [{name}]: '{feature}' does not appear in {binp}.\n"
                          f"        The fuzz test is not registered -- the campaign would report\n"
                          f"        success while fuzzing nothing. Was --features {feature} passed?")
        else:
            notes.append(f"GATE A [{name}]: fuzz test '{feature}' registered")

        try:
            desc = subprocess.run(["file", "-b", bpath], capture_output=True,
                                  text=True, timeout=120).stdout.strip()
        except Exception:
            desc = ""
        if desc:
            if "x86-64" not in desc:
                errors.append(f"GATE D [{name}]: harness is not x86-64 -- workers are amd64 and\n"
                              f"        will never pick it up. `file` says: {desc}")
            elif not any(k in desc for k in ("statically linked", "static-pie linked")):
                warnings.append(f"GATE D [{name}]: harness is NOT statically linked. A glibc build\n"
                                f"        can die on the worker with a bare `status 1`. {desc}")
            else:
                notes.append(f"GATE D [{name}]: static linux/amd64 binary")

        # --------------------------------------------------------- GATE B ---
        if not symp:
            warnings.append(f"GATE B [{name}]: no SymbolsPathInBundle -- no source-level coverage")
            continue
        spath = os.path.join(bundle, symp)
        if not os.path.exists(spath):
            errors.append(f"GATE B [{name}]: symbols missing: {symp}"); continue
        if "/target/" in symp or symp.startswith("target/"):
            errors.append(f"GATE B [{name}]: SymbolsPathInBundle contains 'target/' ({symp}).\n"
                          f"        Crucible splits FUZZ_SYMBOLS at '/target/' to infer the source\n"
                          f"        root, so this corrupts DWARF resolution -> 0 source files.")
        if not orig:
            errors.append(f"GATE B [{name}]: no SourcesOriginalPath -> coverage renders empty"); continue

        pref = orig if orig.endswith("/") else orig + "/"

        # ------------------------------------------------------- GATE E ---
        # Reproduce the SERVER's own check before uploading. The cover task runs
        # `Info.Has(SourcesOriginalPath)` over the LCOV and fails the whole task
        # with "SourcesOriginalPath ... does not match any source file in the
        # coverage profile" when nothing matches -- which surfaces as
        # lines_found: 0 and an empty dashboard, days later.
        #
        # The LCOV keys come from the DWARF compile-unit paths, so check the
        # prefix against those directly. `strings` CANNOT do this: comp_dir is
        # stored once, apart from the relative file names, so it reconstructs
        # paths that do not exist. Use a real DWARF reader, and if none is
        # available say so rather than passing silently.
        dd = None
        for cand in ("llvm-dwarfdump", "dwarfdump"):
            if shutil.which(cand): dd = cand; break
        if dd is None:
            import glob as _g
            hits = sorted(_g.glob("/usr/bin/llvm-dwarfdump-*")
                          + _g.glob("/usr/lib/llvm-*/bin/llvm-dwarfdump"))
            dd = hits[-1] if hits else None
        if dd is None:
            warnings.append(f"GATE E [{name}]: no llvm-dwarfdump; cannot verify that\n"
                            f"        SourcesOriginalPath={orig!r} matches the coverage profile.\n"
                            f"        Install llvm (apt-get install -y llvm) to make this checkable.")
        else:
            # Read the LINE TABLE, not .debug_info. An SF: key is comp_dir (when
            # present) + include_directories[] + file_names[]. It is NOT the
            # compile unit's DW_AT_name: that carries a codegen-unit suffix
            # (.../lib.rs/@/crate.hash-cgu.00) and on real SBF artifacts differs
            # from what lands in the profile -- deriving from it produced a prefix
            # the server then rejected.
            try:
                di = subprocess.run([dd, "--debug-info", spath], capture_output=True,
                                    text=True, errors="replace", timeout=1800).stdout
                dl = subprocess.run([dd, "--debug-line", spath], capture_output=True,
                                    text=True, errors="replace", timeout=1800).stdout
            except Exception:
                di = dl = ""
            comp = {c for c in re.findall(r'DW_AT_comp_dir\s*\("([^"]*)"\)', di)
                    if not re.search(r'\.cargo|/rustc/|toolchain|bpf-tools|platform-tools', c)}
            dirs = re.findall(r'include_directories\[\s*\d+\]\s*=\s*"([^"]*)"', dl)
            keys = set()
            for d in dirs:
                if d.startswith("/"):
                    keys.add(d)
                else:
                    keys.add(d)
                    for c in comp:
                        keys.add(f"{c.rstrip('/')}/{d}")
            if not keys:
                warnings.append(f"GATE E [{name}]: no line-table directories readable from {symp};\n"
                                f"        cannot verify the coverage prefix from here.")
            elif not any(k.startswith(pref) or k.startswith(orig) for k in keys):
                sample = sorted(keys)[:4]
                errors.append(
                    f"GATE E [{name}]: SourcesOriginalPath={orig!r} matches NONE of the\n"
                    f"        {len(keys)} line-table directories in the coverage profile. The server\n"
                    f"        fail the cover task and the project renders lines_found: 0.\n"
                    f"        comp_dir={sorted(comp)[:2] or '<none>'}\n"
                    f"        actual paths e.g. {sample}")
            else:
                hit = sum(1 for k in keys if k.startswith(pref) or k.startswith(orig))
                notes.append(f"GATE E [{name}]: SourcesOriginalPath matches {hit}/{len(keys)} "
                             f"line-table directories")

        blob = strings(spath)
        paths = sorted(set(re.findall(r'[A-Za-z0-9_@./+-]*?[a-z_-]+/src/[A-Za-z0-9_/-]+\.rs', blob)))
        # keep only plausible repo-relative or orig-prefixed paths
        paths = [q for q in paths if len(q) < 300]
        if not paths:
            warnings.append(f"GATE B [{name}]: no *.rs paths recoverable from {symp} via `strings`;\n"
                            f"        cannot verify source resolution from here (not proof of a fault).")
            continue

        srcs_root = os.path.join(bundle, (srcs or "srcs").strip("/"))
        resolved = staged = unresolved = 0
        missing_examples = []
        for q in paths:
            if q.startswith(pref):
                dest = os.path.join(srcs_root, q[len(pref):])
                src_in_repo = os.path.join(repo, q[len(pref):]) if repo else None
                # absolute orig: the repo copy lives at the original location
                if os.path.isabs(orig):
                    src_in_repo = q
            else:
                # Not under the declared prefix. Only FIRST-PARTY sources matter here:
                # the DWARF is also full of Rust stdlib and crates.io paths
                # (../../platform-tools/out/rust/library/..., registry deps), which are
                # not in the repo, are never shipped, and must not be flagged.
                if q.startswith("..") or not repo or not os.path.isfile(os.path.join(repo, q)):
                    continue
                dest = os.path.join(bundle, q)
                src_in_repo = os.path.join(repo, q)
            if os.path.isfile(dest):
                resolved += 1; continue
            if (not check_only) and src_in_repo and os.path.isfile(src_in_repo):
                os.makedirs(os.path.dirname(dest), exist_ok=True)
                shutil.copy2(src_in_repo, dest)
                staged += 1; resolved += 1; continue
            unresolved += 1
            if len(missing_examples) < 5:
                missing_examples.append(os.path.relpath(dest, bundle))
        line = (f"GATE B [{name}]: {resolved}/{len(paths)} DWARF source paths resolve"
                + (f" ({staged} staged now)" if staged else ""))
        if unresolved:
            errors.append(line + f"; {unresolved} UNRESOLVED -> those lines are measured and then\n"
                                 f"        silently dropped. e.g. {missing_examples}")
        else:
            notes.append(line)

for n in notes:    print(f"  ok   {n}")
for w in warnings: print(f"  WARN {w}")
for e in errors:   print(f"  FAIL {e}", file=sys.stderr)
if errors:
    print(f"\nbundle-guard: {len(errors)} blocking problem(s)", file=sys.stderr)
    sys.exit(1)
print(f"\nbundle-guard: OK{' (' + str(len(warnings)) + ' warning(s))' if warnings else ''}")
PY

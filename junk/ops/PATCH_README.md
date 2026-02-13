# JupLend Integration Patch

The file `juplend-integration-full.patch` contains all changes from the JupLend integration work.

## Purpose

If you need to replay the JupLend integration changes on another branch (e.g., after a rebase or merge conflict), you can apply this patch:

```bash
git apply ops/juplend-integration-full.patch
```

Or to apply as commits:

```bash
git am ops/juplend-integration-full.patch
```

## Contents

- 19 commits covering the full JupLend integration
- Rust program changes (instructions, state, errors)
- TypeScript test suite (jl01-jl12 specs)
- IDLs and mocks for JupLend protocols
- Alt-staging deployment scripts and configs

## Starting Point

The patch starts from commit `6d2f6ebf` (just before "first jupiter thing, all tests passing").

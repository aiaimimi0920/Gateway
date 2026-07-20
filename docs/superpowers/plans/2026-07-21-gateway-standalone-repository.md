# Gateway Standalone Repository Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convert the current Gateway snapshot into an independently buildable, releasable GitHub repository and connect it back to Neuro as a submodule.

**Architecture:** Treat `Neuro/Gateway` as the permanent Gateway repository root. Remove parent-relative build and evidence dependencies, add repository-owned automation and metadata, validate an isolated snapshot, then create a fresh `main` history and replace the parent directory entries with a gitlink.

**Tech Stack:** Rust/Cargo, PowerShell, Python unittest, Node.js 22, React/TypeScript, Tauri 2, Docker, GitHub Actions, GitHub REST API, Git submodules.

---

### Task 1: Freeze The Standalone Repository Contract

**Files:**
- Create: `tests/python/test_gateway_standalone_repository_contract.py`
- Create: `docs/superpowers/specs/2026-07-21-gateway-standalone-repository-design.md`
- Create: `docs/superpowers/plans/2026-07-21-gateway-standalone-repository.md`

- [ ] Add assertions for root-local build, package, evidence, metadata, and workflow paths.
- [ ] Run the focused test and confirm it fails on the current parent-relative implementation.
- [ ] Keep the failure output as the red-phase evidence for the migration.

### Task 2: Make Release And Evidence Tools Root-Local

**Files:**
- Modify: `tools/build-gateway-release.ps1`
- Modify: `tools/package-gateway-release.ps1`
- Modify: `tools/run-gateway-line-evidence.ps1`
- Create: `tools/verify-gateway-line.ps1`
- Modify: affected Python contract tests under `tests/python/`

- [ ] Resolve the Gateway root from `tools/..` in every repository-owned script.
- [ ] Change Cargo, executable, desktop, provenance, and source-state paths to root-local paths.
- [ ] Default packages to `<repo>/release/Gateway`; preserve explicit custom output support.
- [ ] copy the Gateway line verifier into the repository and point the evidence runner to it.
- [ ] Update tests to resolve `GATEWAY_ROOT` directly rather than the Neuro parent.
- [ ] Run the focused tests until green.

### Task 3: Add Repository Metadata And Automation

**Files:**
- Create: `.gitattributes`
- Modify: `.gitignore`
- Modify: `.dockerignore`
- Create: `LICENSE`
- Create: `README.zh-CN.md`
- Modify: `README.md`
- Create: `.github/workflows/ci.yml`
- Create: `.github/workflows/build-windows.yml`
- Create: `.github/workflows/docker.yml`
- Create: `.github/workflows/release-tag.yml`
- Create: `tools/compress-gateway-release.ps1`

- [ ] Add cross-platform line-ending and binary rules.
- [ ] Ignore all repository-local generated outputs while retaining local development state.
- [ ] Add independent clone/build/test/package documentation and repository badges.
- [ ] Add Windows/Linux CI, Windows artifact build, Docker/GHCR, and validated tag release workflows.
- [ ] Add deterministic ZIP and SHA-256 creation for release assets.
- [ ] Run the workflow/metadata contract test until green.

### Task 4: Validate The Complete Snapshot

**Files:**
- Modify only files required by failing validation.

- [ ] Run all Python tests from the Gateway root.
- [ ] Run all Node browser-worker tests.
- [ ] Run `cargo fmt --all -- --check`, `cargo check --locked --all-targets`, and `cargo test --locked`.
- [ ] Run desktop `npm ci`, typecheck, and build.
- [ ] Run Tauri format and check.
- [ ] Build the Docker image.
- [ ] Run release-script dry runs with the explicit Neuro release root policy.
- [ ] Copy the candidate tracked snapshot to a temporary isolated directory and rerun standalone contracts.

### Task 5: Create And Push The Independent Repository

**Files:**
- Create: `Gateway/.git/` via `git init -b main`

- [ ] Confirm ignored credentials and generated output are not staged.
- [ ] Confirm all current source, docs, tests, scripts, and product work are staged.
- [ ] Create the initial snapshot commit.
- [ ] Create private `aiaimimi0920/Gateway` through GitHub REST using only a process environment token.
- [ ] Add the clean HTTPS remote and push `main`.
- [ ] Confirm remote visibility, default branch, commit, and repository files through the API.
- [ ] Fresh-clone the remote and rerun fast standalone validation.

### Task 6: Convert The Neuro Parent To A Submodule

**Files:**
- Modify: `../.gitmodules`
- Replace parent tracked `Gateway/**` entries with the `Gateway` gitlink.

- [ ] Remove Gateway paths from the parent index only; do not delete the working directory.
- [ ] Add the Gateway gitlink and `.gitmodules` entry.
- [ ] Commit only the Gateway submodule conversion and `.gitmodules` update.
- [ ] Confirm unrelated parent worktree changes remain untouched.
- [ ] Confirm `git submodule status Gateway` matches the pushed child commit without a `+` prefix.

### Task 7: Final Audit

**Files:**
- Modify only files needed to resolve verified defects.

- [ ] Review the independent diff and workflow contracts.
- [ ] Verify protected source hashes remain unchanged.
- [ ] Verify the backup and existing `release/Gateway` product package remain intact.
- [ ] Query GitHub repository and Actions configuration without exposing credentials.
- [ ] Report repository URL, commits, verification commands, and any residual external Actions status.


# Gateway Standalone Repository Design

**Date:** 2026-07-21

**Repository:** `aiaimimi0920/Gateway`

**Parent integration:** `Neuro/Gateway` remains at the same filesystem path and
is represented by a Git submodule entry in the Neuro repository.

## Goal

Move the complete current Gateway working tree into a dedicated Git history and
GitHub repository without losing the productization work that is not yet in the
Neuro parent repository's HEAD. Make a fresh clone independently buildable,
testable, packageable, and releasable.

## Safety And Scope

- The verified pre-migration backup at
  `C:\Users\Public\nas_home\AI\GameEditor\_temp\Gateway-backup-20260721-023002`
  is the recovery baseline.
- Existing credentials, `.env`, routes, account material, and development state
  are not deleted or rewritten.
- `.env` stays local and ignored. Existing versioned development configuration
  is preserved unchanged.
- The initial GitHub repository is private while development credentials and
  configuration remain in use. Visibility can be changed after the owner
  completes the final credential cleanup.
- No unrelated Neuro, Hook, Loom, Talk, Platform, or Tea changes are modified.
- Local release packages produced during this conversation remain under
  `C:\Users\Public\nas_home\AI\GameEditor\Neuro\release\Gateway`.

## Migration Strategy

Use the same snapshot-history pattern proven by Hook:

1. Adapt the current Gateway working tree to run from its own repository root.
2. Validate the complete current snapshot, including uncommitted product work.
3. Initialize a new `main` history in the existing `Neuro/Gateway` directory.
4. Create and push `aiaimimi0920/Gateway` using process-only authentication.
5. Replace the parent's ordinary tracked directory entries with a mode `160000`
   gitlink and add the Gateway entry to `.gitmodules`.

History filtering is intentionally not used. The parent HEAD does not contain
all current Gateway files, so a path-filtered history would silently omit part
of the productized implementation.

## Standalone Contract

The Gateway repository root contains `Cargo.toml`, `Dockerfile`, `README.md`,
`src/`, `tools/`, `scripts/`, `manifests/`, `tests/`, and `apps/desktop/`.

- Rust commands run directly from the repository root.
- Desktop commands run from `apps/desktop`.
- Browser worker commands run from `scripts`.
- Repository-only evidence tools resolve all dependencies below the Gateway
  root. The provider line verifier is owned by Gateway rather than `../deploy`.
- Packaging defaults to `release/Gateway` inside a standalone checkout and also
  accepts an explicit release root for the Neuro workspace policy.
- Generated targets, runtime state, dependencies, local releases, and local
  credentials remain ignored and are not part of the independent snapshot.

## GitHub Automation

Four workflows provide the product baseline:

- `ci.yml`: Windows and Linux validation on pushes, pull requests, and manual
  dispatch. It checks repository contracts, Python tools, Node workers, Rust,
  the desktop frontend, and the Tauri wrapper.
- `build-windows.yml`: builds and verifies a Windows release candidate and
  uploads the immutable package directory as an Actions artifact.
- `docker.yml`: builds the Dockerfile for pull requests and pushes; pushes to
  `ghcr.io/aiaimimi0920/gateway` only from `main` or an explicit tag.
- `release-tag.yml`: accepts only `Vx.y.z`, runs validation, builds the Windows
  package, creates a ZIP and SHA-256 file, and publishes a GitHub Release.

Workflow permissions are job-scoped and minimal. Release publication never
bypasses validation.

## Verification

Before the remote repository is created:

- Run the new standalone repository contract tests and all Python tests.
- Run browser worker tests.
- Run Rust format, check, and tests.
- Run desktop typecheck/build and Tauri check/format.
- Build the Docker image.
- Copy the tracked candidate snapshot to an isolated temporary directory and
  rerun root-layout and dry-run release checks there.

After pushing:

- Clone the remote repository into an isolated temporary directory.
- Confirm the remote default branch, visibility, commit, and clean checkout.
- Run the fast standalone contracts from the fresh clone.
- Confirm the Neuro parent records Gateway as a submodule pointing at the exact
  pushed commit.


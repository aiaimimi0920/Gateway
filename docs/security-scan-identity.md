# OSV scan identity and scan-only validation

GitHub reported `configuration not found: .github/workflows/build-windows.yml:osv-scan`
on [the PR #17 code-scanning check](https://github.com/aiaimimi0920/Gateway/runs/111045791858).
Main invoked OSV through Build Windows, while PRs lacked that caller. Running the
same scanner directly through Security does not supply the missing caller identity.

Build Windows now scans every PR targeting main, without path filters, through
the unchanged `build-windows.yml` -> `security.yml` -> pinned OSV reusable workflow
chain. Job keys remain `security`, `dependencies`, and upstream `osv-scan`.
The wrapper has no category input; no invented input or replacement upload is used.
GitHub's actual analysis key/category and upload processing must be checked on the
exact PR merge SHA, rather than inferred from matching YAML or a generic banner.

## Build boundary

The security caller still receives `github.sha`, resolves it once, and scans that
immutable revision. The build checks out `needs.security.outputs.source_sha`.
It requires Security success plus either a push to `refs/heads/main`, or an
explicit manual dispatch with boolean `scan_only: false`. Manual dispatch defaults
to `scan_only: true`; PRs never enter the Windows build job. `toJSON` preserves the
input type so missing, null, numeric and string values cannot opt into a build.
See [GitHub's expression semantics](https://docs.github.com/en/actions/reference/workflows-and-actions/expressions).

The three-file change does not alter products, dependencies, suppressions,
scanner pins, permissions, or Docker/tag-release gates. PR validation requests no
release, signing or deployment. Existing Docker PR runs remain subject to their
Security dependency and cannot publish an image on a PR event.

**Do not auto-merge this repair.** A merge pushes main: after Security succeeds,
Build Windows may package/upload candidates, and the existing Docker workflow may
publish to GHCR. Tag publication has its own existing trigger and is unchanged.
Review those main-push side effects before any later merge decision.

## Findings and coverage limits

The five-lock scan and `fail-on-vuln: true` stay intact. Existing desktop findings
`glib 0.18.5` (`RUSTSEC-2024-0429` / `GHSA-wrw7-89jp-8q8g`) and
`proc-macro-error 1.0.4` (`RUSTSEC-2024-0370`) are not ignored or downgraded.
Fixing configuration identity is not fixing either advisory.
Dependabot's `security_update_not_possible` for glib (resolvable 0.18.5 versus
minimum safe 0.20.0) is a dependency-resolution limitation, not this configuration
error. A dependency migration needs its own review; this change does not force it.

The pinned v2.5.1 wrapper tolerates the scanner step's error before its reporter
runs. Scanner-error/missing-JSON reporter handling is a separate known hardening
task owned elsewhere; this repair does not claim to close it. A green wrapper
alone is insufficient evidence that a scan completed. Check actual scan output,
findings, SARIF upload and GitHub processing; fail-on-finding only protects findings
the wrapper successfully reports. This limitation matters before any publication.

## Verification

Local contracts pin the exact build guard and retain every other caller's gate.
Negative mutations reject failed-scan bypasses, PR builds, non-main pushes and
loose input comparisons. The actual allowlisted expression is evaluated across
900 event/ref/result/input combinations; this is not a GitHub Actions emulator.

For remote acceptance, record the PR head and merge SHA, Build Windows run URL,
resolved checkout SHA, OSV analysis key/category and completed SARIF processing.
Confirm Windows build is `skipped`, scan/reporter output contains the real
advisories and the finding check remains red. Report any pending/unavailable
analysis evidence explicitly. No native repository security settings need changing.

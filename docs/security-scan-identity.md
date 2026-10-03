# Development reports and strict release gates

GitHub previously reported `configuration not found: .github/workflows/build-windows.yml:osv-scan`
on [PR #17's code-scanning check](https://github.com/aiaimimi0920/Gateway/runs/111045791858).
Main invoked OSV through Build Windows, while PRs lacked that caller. Build Windows
now scans every main PR without path filters, retaining job keys `security`,
`dependencies`, and terminal `osv-scan`. The last job now belongs to a local reusable
workflow; upload-sarif supplies its native identity without an invented category input.
Acceptance requires the actual analysis key and completed upload on the exact merge SHA.

## Development policy

Valid findings are reports for follow-up work, not failures of functional development.
Tool execution, parsing, incomplete reports and upload failures remain failures in
independent jobs. Product CI has no dependency on the reporting jobs.

| Check | Development behavior | Execution errors |
| --- | --- | --- |
| OSV | Complete JSON and SARIF, native alerts and summary; findings advisory | Failed job |
| Gitleaks | Full checked-commit history, redacted SARIF artifact and count summary | Failed job |
| npm production audit (both roots) | Complete audit JSON artifact and count summary | Failed job |
| Rust/Tauri formatting | Successful formatter followed by complete changed-file/count report | Failed job, including syntax errors |
| Effective code lines | Existing ratchet, complete JSON independently checked against a fresh source scan | Failed job, including baseline/integrity/coverage errors |
| CodeQL | Existing extended analysis and full native alerts | Analyze/upload failures remain red |
| Compile, typecheck, unit/runtime tests | Existing required product validation behavior | Failed product job |

CodeQL's native code-scanning check can still report new findings under repository
rules. This patch does not change protection settings, severity, query coverage or
alert visibility. A native finding-only result and an execution failure must be
assessed separately; changing repository-required checks needs separate authorization.

Reports use job summaries, artifacts and existing alerts instead of duplicate issues
on each run. No issue/comment write permission or new token is introduced. Reported
findings remain available for separate fixes. Logs and summaries never print raw
secret findings; Gitleaks artifacts retain rule, commit and source location while
removing snippets and commit-message/person metadata.

## Strict result classification

The old OSV 2.5.1 reusable wrapper continued after scanner errors; its reporter could
accept missing JSON. Gateway bypasses that wrapper with the same official scanner
version, downloaded with a committed SHA256 checksum. This is Gateway-local hardening,
not a claim that the upstream wrapper or another owner's project has been repaired.

OSV runs at error-only verbosity with all five locks, all packages and all findings.
Both JSON and SARIF runs must finish, have no error diagnostics, and return 0 for clean
results or 1 for findings. Missing/invalid JSON, incomplete lock coverage, missing
vulnerability groups, SARIF omissions, mismatched package/version/location and all
other exits fail. Both runs must agree; database drift between them fails validation
rather than silently dropping a finding. Upstream emits one SARIF occurrence per
advisory, including aliases; the validator preserves this multiplicity.

Gitleaks 8.24.3 is also checksum-verified. It uses its explicit `--exit-code=2` for
findings so ordinary error exit 1 cannot be mistaken for a finding. SARIF must be
valid, redacted, consistent with the exit status, and have no execution diagnostics.
The npm audit validator checks schema, severity counts and complete records; exit 1
is accepted only with validated high/critical findings. Low findings can accompany
exit 0 at the existing high audit threshold. Formatters must return 0 before a Git
diff is accepted; an exit 1 is never interpreted as a formatting finding.

Each report upload is an ordinary required step. OSV waits for SARIF processing.
Strict finding enforcement occurs after evidence uploads so real findings survive
release rejection. No blanket `continue-on-error` or fallback empty report is used.

## Publication boundary

The reusable Security workflow defaults to strict mode for workflow callers.
Standalone development Security scans are advisory; Build Windows explicitly selects
advisory for PRs and manual `scan_only: true` runs. Docker PRs also use advisory
scans and npm audits, including the two Dockerfile stages; valid findings permit
image/stack verification. Every other Docker event and Release Tag remain strict.
Dockerfile audits default to strict; only the PR verification build explicitly passes
advisory mode. The publishing build explicitly passes strict mode. Both modes run
the audit and validate complete JSON; neither suppresses tool/report failures. Host
reports are uploaded as artifacts, and each image stage retains its own audit JSON.
Strict host audits retain any completed report on failure; as before, a strict
failure stops later build steps. PR advisory findings allow both audits to finish.

Windows and tag publisher jobs require Security success **and** `findings_free == 'true'`.
Docker requires the same except for ordinary PR verification, which cannot log in or
push an image.
The output is true only when dependency and secret reports are both validated clean.
Missing outputs cannot authorize publication. They build the immutable commit
resolved by Security, not a mutable branch ref.

Windows candidate builds additionally require a main push or manual dispatch with
boolean `scan_only: false`. The default is true; missing, null, numeric and string
values cannot opt into building. PRs never enter this build job.

Merging still triggers main workflows: if strict scans are genuinely clean,
Build Windows can package/upload candidates and Docker can publish to GHCR under its
existing main/tag/manual conditions. The current real findings keep those strict
paths blocked. This development policy does not authorize tags, signing, deployment,
protection changes, suppression of findings or dependency/product edits.

## Existing findings and validation

`glib 0.18.5` (`RUSTSEC-2024-0429` / `GHSA-wrw7-89jp-8q8g`) and
`proc-macro-error 1.0.4` (`RUSTSEC-2024-0370`) remain in complete reports. Repairing
configuration or making development reporting advisory does not fix either issue.
No ignores, baselines or severity overrides are added. Dependency migration is separate.

Focused tests cover clean and vulnerable reports, alias occurrences, multi-version
advisories, incomplete/malformed output, error diagnostics, unknown exits, unavailable
tools, redaction, npm thresholds, formatter parse failures, complete line-audit
reconciliation, and upload-before-enforcement ordering. The exact Windows guard is
evaluated across 4,500 event/ref/result/input/clean-output combinations with mutation
negative tests; this is not a GitHub Actions emulator.

For remote acceptance record head, merge SHA, run/job links, resolved scan checkout,
actual `.github/workflows/build-windows.yml:osv-scan` analysis key and completed SARIF
processing. Verify development findings succeed with reports, Windows build stays
skipped, and Docker publishing steps stay skipped. Check independent
quality reports and real product CI separately. Docker PR verification must run
without registry login/push; main/tag/manual strict paths must reject findings.
Never substitute an old-head run or
a generic native banner for those details.

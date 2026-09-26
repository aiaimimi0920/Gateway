# Gateway Effective-Code-Line Policy

Gateway follows the Neuro incremental development standard in
`../docs/DEVELOPMENT_STANDARD.md` when mounted in the Neuro workspace. This
document explains the repository-local machine gate that makes the size rules
enforceable in standalone Gateway checkouts.

## Authority

The machine-authoritative files are:

- `scripts/effective-code-lines.mjs`
- `scripts/effective-code-lines-lexer.mjs`
- `scripts/effective-code-lines-runtime-artifacts.mjs`
- `scripts/effective-code-lines-policy.json`
- `scripts/effective-code-lines-baseline.json`
- `scripts/effective-code-lines-exceptions.json`

If prose and the checker disagree, obey the checker and correct the stale prose
in the same change. Do not edit the checker, policy, baseline, or exceptions
merely to hide a violation.

## Effective lines and thresholds

The language-aware lexer removes blank lines, comment-only lines, and
comment-only multiline regions. A line with code and an inline comment counts.
String, raw-string, template, regex, here-string, heredoc, and SQL string content
remains code.

The checked tiers are:

| Effective lines | Classification | Required action |
| ---: | --- | --- |
| About 150 | Design target | Prefer a focused 100-250 line responsibility. |
| 251-500 | Acceptable | Keep one clear, cohesive responsibility. |
| 501-700 | Soft exception | Split or record an exact reviewed exception. |
| 701-1500 | Mandatory split | New files cannot finish in this range. |
| More than 1500 | Hard cap | New files have no waiver and must be split again. |

The policy scans handwritten Rust, TypeScript/TSX, JavaScript/JSX/MJS/CJS,
CSS/SCSS, HTML, PowerShell, Python, CMD/BAT, POSIX shell, and SQL. Explicit
exclusions are limited to generated output, installed dependencies, local
runtime state, framework-generated files, and immutable machine-generated
artifacts with a reason in the policy.

## Adoption baseline and ratchet

Gateway had substantial historical file-size debt when the machine gate was
introduced. The checked-in baseline records every adoption-snapshot file above
500 effective lines with its exact path, line counts, and normalized-source
SHA-256. A baseline entry is debt evidence, not proof that the file is
well-sized.

The canonical digest of the complete baseline file list is anchored in the
policy. The recorded Git commit and tree are provenance for the adoption point;
because the initial adoption included an existing dirty working tree, the exact
per-file hashes—not the clean commit tree—identify the grandfathered snapshot.

The normal `ratchet` gate applies these rules:

- a file above 700 that is absent from the baseline fails;
- a baseline file above 700 may receive a narrow in-place correctness, security,
  compatibility, or wiring fix only when its effective-line count does not grow;
- reducing an oversized baseline file while it remains above 700 is allowed and
  reported as debt;
- a new or changed 501-700 file requires a current reviewed exception;
- renaming or moving an oversized file does not preserve its baseline identity;
- files at 500 or fewer do not need an exception, but still require one clear
  responsibility.

The machine gate cannot decide whether a same-size edit introduced a new
responsibility. Reviewers must still enforce the ownership and anti-gaming rules
in `AGENTS.md`.

`strict` reports every governed source file above 700 as a violation. It is the
migration-closure target, not the ordinary CI mode while recorded debt remains.

## Immutable browser runtime artifacts

Checker version 2 accepts the policy's optional `immutableRuntimeArtifacts`
reference only when its exact registry path and SHA-256 bind a reviewed provenance
receipt. The [registry](plan/runtime-profile-artifacts-2026-09-24.json) individually
names fourteen installed Suno/Udio package assets. Its receipt records authenticated
package identity, signed-byte checks and negative controls; the checker validates
that binding and the current raw bytes. No directory exclusion is added.

Matching assets remain measured in `summary` and the oversized `files` list. The
report additionally exposes `runtimeArtifacts` and `sourceSummary`; strict and
ratchet evaluate the governed source class. A registered byte change fails with a
diagnostic, including line-ending drift. Unlisted siblings, new versions and
first-party files follow the ordinary rules. Missing optional payloads are allowed
in a clean checkout; missing or malformed registry/receipt evidence fails closed.

Keep installed payloads immutable and upgrade them through their package owner.
The classification concerns local source measurement and establishes no right to
redistribute a complete browser package. Any registry, receipt, checker or policy
change requires reviewed integrity metadata, preserving the adoption records unless
a separate new snapshot is explicitly authorized. The precise migration and its
evidence are recorded in the [governance proposal](plan/runtime-profile-governance-proposal.md).

## Commands

Run from the Gateway repository root:

```powershell
npm run test:effective-lines --prefix scripts
npm run check:effective-lines --prefix scripts
```

Generate a non-blocking debt report:

```powershell
npm run report:effective-lines --prefix scripts
```

Audit the repository against the strict 700-line ceiling:

```powershell
npm run strict:effective-lines --prefix scripts
```

CI runs the checker tests and the ratchet on both Windows and Linux and uploads
the JSON report even when a later product gate fails.

## Soft exceptions

An entry in `scripts/effective-code-lines-exceptions.json` is valid only for a
current 501-700 line file and must contain:

- repository-relative path;
- exact effective-line count;
- exact normalized-source SHA-256;
- one responsibility statement;
- a concrete cohesion reason;
- owner;
- independent approver different from the owner;
- unexpired `reviewBy` date in `YYYY-MM-DD` format;
- non-empty protecting tests.

Any source change invalidates the exception hash. Coding agents must not invent
an owner, approver, approval decision, or protecting test result.

## Baseline maintenance

The adoption baseline is not a convenience file and is not regenerated during
ordinary feature, fix, or refactor work. A CI failure must be resolved by
splitting the new responsibility, removing legacy growth, or supplying a valid
501-700 exception.

Only an explicit repository-governance migration may create a new adoption
snapshot. The command deliberately requires a second acknowledgement flag:

```powershell
node .\scripts\effective-code-lines.mjs --write-baseline --acknowledge-adoption-snapshot
```

Review the complete baseline diff, source paths, counts, hashes, policy
exclusions, and the recorded source commit/tree before accepting it. Never use
baseline regeneration to make an unrelated task green.

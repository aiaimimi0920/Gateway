# Implementation-line ownership

Accepted: 2026-09-13 01:29:48 UTC. Gateway-only structural checkpoint.

The entry decreases from 1,585 effective lines to 114. Catalog metadata, profile
lookup and adapter-first payload inference now have private cohesive owners. The
entry retains compile enforcement, error construction and stable public exports.
The original tests are grouped by their contracts; no behavior changes are part
of this extraction.

| Source | Effective lines |
| --- | ---: |
| src/implementation_lines.rs | 114 |
| src/implementation_lines/catalog.rs | 325 |
| src/implementation_lines/profiles.rs | 186 |
| src/implementation_lines/payload.rs | 117 |
| src/implementation_lines/tests.rs | 7 |
| src/implementation_lines/tests/fixture.rs | 38 |
| src/implementation_lines/tests/feature_profiles.rs | 142 |
| src/implementation_lines/tests/profiles.rs | 143 |
| src/implementation_lines/tests/payloads.rs | 314 |
| src/implementation_lines/tests/compile_checks.rs | 243 |

Exact formatted-source comparison preserves 65 production definitions, 64 public
entry paths, three public enum methods, 31 explicit test bodies and fourteen
unchanged macro invocations. Those invocations generate 28 conditional test names;
the paired feature configurations cover all 59 distinct line-test identities.
The fixture body is unchanged; only make_payload gains test-family visibility.
All 361 neighboring inputs, including 31 discovered caller files, Cargo manifests,
feature declarations and protocol-registry contracts remain byte-identical.

Feature constants, enum variants, profile aliases, adapter precedence, URL
substring classification, allocation, ordering, unknown-profile handling and
error status/code/text are unchanged. These pure owners do not transfer a process,
connection, task, cancellation path or cleanup obligation. URL classification and
allocation remain separate optimization decisions after this structural proof.

Paired baseline and final gates pass:

| Gate | Baseline | Final |
| --- | ---: | ---: |
| Default implementation-line tests | 37/37 | 37/37 |
| Default official-profile registry tests | 7/7 | 7/7 |
| No-default-feature implementation-line tests | 41/41 | 41/41 |

Test-name sets match after ignoring only the new nested module paths. Each phase
covers all 59 line-test identities across the two feature configurations. Warning
sets for the paired test gates are unchanged. Fresh offline locked all-targets
compilation passes with the three existing upstream warnings. All Cargo gates are
serialized and terminal.

Scoped rustfmt, checker 19/19, adoption ratchet, source/encoding proof, Cargo
cleanup and both repositories' git diff --check pass. Global formatting still
reports only the two unchanged S06 runtime-mirror files. Strict scans 1,635 files:
28 hard, 27 mandatory and 40 soft. There are 55 files above 700, down from 56;
clearance advances to 90/145 (62.1%). Every extracted owner is below 500.

Immutable evidence is
target/effective-line-evidence/20260913-implementation-line-owners/scope.json,
SHA-256 90e70606820d2ce8e1dc3cced1d71fd1997e569c76c72355e3fcdfdbd51b2e01.
The initial raw-CRLF versus canonical-LF hash preparation error is recorded in
before.json; both before/ and before-v2/ preserve the original source bytes. The
first acceptance attempt incorrectly expected sub-500 owners in the checker's
debt-only files array. capture-scope-attempt1.json records that helper correction;
acceptance remeasures each owner with the repository lexer and canonical hash.
No source or Cargo receipt changed for either evidence-preparation correction.

At acceptance Gateway has 1,813 status entries: 176 modified, one unstaged
deletion, two staged deletions and 1,634 untracked. Neuro has 192 entries: ten
modified and 182 untracked. Both HEADs and inherited deletions are preserved.
These counts precede this final report update.

Preset ownership is the next structural review. S06 keeps its implementation and
cursor. GWP-20260912-01, its two formatter findings, runtime-profile governance
and final freeze/build transfer remain pending. Overall plan completion still
requires zero strict debt above 700, valid soft exceptions, full language/provider
and release gates, immutable packaging, packaged runtime/UI/Docker acceptance and
the persistent 4200 service without persistent 4226. No release, persistent
deployment, dependency, baseline, exception or checker-policy change was made.

# Udio worker and S06 integrated interim release

Version:20260908-udio-worker-s06-051500. Verified2026-09-08 at05:44UTC.
Release root:C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway/.
This is an immutable partial milestone, not completion of the optimization goal.

## Included checkpoint

- Lane F:34 AI Studio probe tests split into cohesive sibling test owners.
- Lane G: Udio production worker1754 ->486 effective entry,11 modules <=321.
  Exact55-function pure-move proof preceded separate lifecycle, local storage
  containment/atomic replacement and16MiB serialized-state hardening.
- S06 frozen bounded-upload diagnostic batch and its ingress tests, accepted
  from the05:01UTC ownership transfer. S06 reports Gemini588 passed,3 ignored,
  all-target check passed; those reports are not a coordinator full Rust RC run.

## Fresh evidence

Evidence directory:target/release-evidence/20260908-udio-worker-s06-051500/.
Source fingerprint before/after official build, provenance and package agree:

    496a837441b50da80a3fa4ffdc8f489db5af65480ffc9a9c33d9b9fdd5260b64

Source census1671 files; dirty=true is intentional and includes untracked owners.
Subsequent documentation edits do not alter this immutable recorded snapshot.

| Gate | Actual result |
| --- | --- |
| Integrated Node |165/165 passed; explicit test-file enumeration |
| Package Python contracts |23/23 passed |
| Udio Python contracts |7/7 passed |
| Checker/ratchet |19 checker tests passed; adoption ratchet passed |
| Strict |1038 scanned;44 hard+77 mandatory=121 above700;38 soft; expected exit1 |
| npm production audits |0 vulnerabilities in workers and desktop |
| Desktop typecheck/web build |Passed |
| Headless Rust release |Passed,21m50s; three existing compiler warnings |
| Tauri release |Passed,2m43s compiler time |
| Package/UI integrity |Passed; UI was not launched |
| Packaged runtime, two runs |All10 checks passed in each; drain and exit0 |
| Cleanup |Both disposable Redis containers removed; packaged processes0 |
| Existing Docker census |All47 identities/names/states preserved |
| Final exact package integrity |Passed after both runtime runs |

Runtime ports52648/52670 and disposable Redis52649/52674 were isolated from
the persistent stack. Inherited database URLs were removed only in the smoke
helper process. No live4200 deployment, existing release or provider profile changed.

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| gateway.exe |56997888 |fa2d9ae593db1f7682ae87c713d0f0d7ffeb6afe79863c3167941959190b9c97 |
| gateway-ui.exe |11935744 |e59ba00a844586ffd6817802c02974f100647c8d4b60557854a09bca74df4387 |

Manifest SHA-256:
ae612730faa2ef92774a9a982ddeb4400790b74e5f371ae34f39e12fa916ca71

Checksums file SHA-256:
c90dc376813609c176057c3a46dfc4757db575a0058a6e8e14b87b595477233d

## Remaining acceptance work

Original debt reduction remains24/145 (16.6%). Lane H worker is unchanged.
Udio shared-context origin/export scope, cookie fallback domain filtering,
diagnostic redaction, response bounds and subprocess cancellation are unfinished.
16MiB state collection is not a total-memory or execution-time guarantee.
S06 concurrency limits are not decoded-pixel/process-memory/time limits.
There was no visual UI E2E, live provider semantic run, live S3 test, complete
Python/Rust RC matrix, or final persistent-stack4200/no4226 acceptance this round.
See parallel-lanes/udio-worker.md and the original S00-S21 plan; do not infer
full S06, S11 or S21 completion from this interim release.

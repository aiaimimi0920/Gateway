# Lane M: standalone repository contract decomposition

Coordinator test-only scope. Production publishers, Rust/S06, real Git history,
live Docker and immutable releases remain untouched by this lane.

## Part 1: source provenance contracts

The original suite1975 effective lines now retains1674 and is still hard debt.
Six cohesive source/build-root/fingerprint contracts moved intact into
`test_gateway_standalone_source_state.py` (288 effective lines). A non-TestCase
`gateway_repository_text_fixture.py` (30) owns the three unchanged text-block
and marker-order assertion helpers needed by both suites. No tests are inherited
from the fixture and no test body is duplicated.

All61 original class methods and their decorators match exactly after the move:
58 test methods plus3 helpers. Source root resolution stays at parents[2].
The source-state tests use synthetic temporary Git repositories; their fixture
commits do not touch Gateway Git history. No build command is actually invoked
by the moved tests, which execute extracted PowerShell source-state functions.

Evidence root: `target/effective-line-evidence/standalone-tests/`.

- `before.py`: original pre-edit source.
- `source-state-before.log`:3/3 baseline,22.459s.
- `build-root-before.log`:1/1 baseline.
- `part1-after.log`: all58 tests pass,47.227s, before moving the final2 provenance
  text contracts into their cohesive owner.
- `part1-final.log`: all58 tests pass,44.416s, final6-test owner.
- `part1-final-ratchet.log`: ratchet pass. AST method/decorator equality,
  UTF8-no-BOM and Git diff checks also pass.

This is deliberately an incomplete migration checkpoint, not cleared debt.
The remaining original suite must be decomposed by CI/dependency, web asset
publication, Docker package and deployment contract responsibilities before
lane M can be accepted. Do not increment the29/145 clearance count yet.
No source/docs freeze receipt for GWP05 has arrived; no shared build initiated.

## Final structural and discovery closure

The remaining responsibilities have now moved into cohesive suites. The part1
unfinished status above is historical, superseded by this measured checkpoint:

| Owner | Effective lines | Tests |
| --- | ---: | ---: |
| Original repository/package contracts | 479 | 15 |
| CI/dependency/build-stage contracts | 300 | 12 |
| Deployment/helper contracts | 224 | 13 |
| Source provenance | 288 | 6 |
| Web readiness/lock ownership | 343 | 6 |
| Web transactions/pruning/rollback | 375 | 7 |
| Non-TestCase text fixture | 30 | 0 |

All61 original methods/decorators remain exact, including all embedded JS/PS
fixtures and58 original tests. One obsolete re import was removed. Independent
review confirmed discovery naming and fixture ownership, but found the Linux
CI standalone gate still naming only the old module (omitting43 relocated
tests). A separate regression failed against that command. The single CI step
now uses `python -m unittest discover -s tests/python -p
"test_gateway_standalone*.py" -v`, which includes all59 tests. This one-line
workflow wiring fix is the only scope extension beyond tests/docs; no runtime
publisher or build implementation changed.

Evidence: part2-after58/58 in45.630s, part3-after58/58 in46.422s, final58/58
in44.625s, discovery-before1failure and discovery-final59/59 in44.966s.
Final ratchet and Git diff checks pass, all7 source/test files are UTF8 without
BOM. Strict1104 scanned:41 hard+74 mandatory=115 above700,38 soft, exit1.
This clears one original oversized file:30/145 (20.7%) structurally cleared.

These contracts use synthetic temporary Git/Node/PowerShell fixtures, not a
product build, deployed-stack or hosted-CI proof. Existing process fixture
timing assumptions were preserved, not newly certified race-free. No immutable
release was altered; GWP05 source/docs freeze and Cargo receipt remain pending.

## Expanded offline gate follow-up

Full Python discovery initially ran282 tests in482.893s:7 failures,4 skips.
Four stale Suno entry-file assertions were corrected separately (focused5/5).
The remaining three Docker dependency contracts confused definition order with
call order, or expected the retired Cargo log prefix. Function-scoped assertions
now verify preparation, both Node installs/audits, watcher order and both main
watch branches. Focused23/23 pass; a synthetic reordered frontend installation
is rejected. Production Docker code and its opt-in audit default are unchanged.
Evidence: docker-entrypoint-before/after/final.log and docker-order-mutation.log
under this lane's evidence directory. A full offline rerun is still pending;
this is test-contract repair, not a new runtime or immutable-release acceptance.

Terminal rerun receipt: full-python-after.log reports283 tests in483.761s,
zero failures and4 skips with all three live-E2E opt-ins disabled. Node matrix
node-final.log reports290 passed and1 POSIX skip (291 total). Focused Docker23/23
also covers the final stricter frontend installation marker. Checker, ratchet
and Git diff checks pass. Source remains unreleased; these offline contracts do
not establish Docker execution, hosted CI, provider behavior or S21 completion.

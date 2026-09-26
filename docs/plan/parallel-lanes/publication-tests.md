# Lane L: release publication test decomposition

Scope: Python contracts and their temporary fixtures only. No production
publisher, Cargo, live stack or immutable release was changed by this lane.

The original 1192-effective-line suite now has four responsibility owners:

- `gateway_release_publication_fixture.py`: 461 effective lines, shared process,
  synthetic package and lock fixtures; not a TestCase.
- `test_gateway_release_publication_contract.py`: 184, four platform, immutable
  artifact and portable archive contracts.
- `test_gateway_release_publication_concurrency.py`: 328, four simultaneous
  publication, staging isolation and process/crash contracts.
- `test_gateway_release_publication_recovery.py`: 289, four interrupted journal,
  checksum and cleanup-recovery contracts, retaining the Windows skip decorator.

## Structural checkpoint

Before subsequent test synchronization changes, AST source plus decorators
matched exactly for all 31 methods: 19 fixture helpers and 12 tests, no duplicates.
Pure-move sizes were 446/184/312/289. Original baseline ran 12 tests in 120.830s
with one failure waiting for `staging-holder.ready`; the pure split ran all 12
successfully in 112.117s. A single passing rerun did not resolve that baseline race.

## Separate synchronization correction

The isolation test previously observed staging and only then launched a new
PowerShell holder. The publisher could finish and delete staging during process
startup. A large random payload was not a synchronization guarantee.

An opt-in hook now instruments only the copied temporary Docker publisher after
directory creation. The production script is untouched and the insertion anchor
must occur exactly once. Each publisher has its own ready/gate paths; fixture
waiting is bounded to 60 seconds. A stays paused until its holder is established
and B has created separate staging. The test explicitly checks A still exists
and the holder is alive. It releases the holder before allowing either publisher
to finish, avoiding dependence on cleanup retry timing or unlink semantics.
Existing archive, checksum, immutable-output and residue assertions remain.
All owned child processes still terminate in the test's finally block.

An independent read-only review informed the added A-survival assertion and
release ordering. No claim of Linux runtime validation follows from Windows runs.

## Fresh verification

Evidence: `target/effective-line-evidence/publication-tests/`.

- `before.py`, `before.log`: original source and baseline failure.
- `after.log`: pure split, 12/12.
- `barrier.log`: initial bounded fixture barrier, 12/12 in 112.985s.
- `barrier-final.log`: final ordering and assertions, 12/12 in 118.808s.
- `barrier-repeat.log`: same-version filtered repeat, 2/2 in 18.473s.
- `node-final.log`: 249/249 Node tests, including current ChatGPT capture bounds.
- `checker-final.log`: 19/19 lexer/checker tests.
- `barrier-ratchet.log`: ratchet exit 0; Git diff check exit 0.
- `strict-final.log`: 1094 scanned, 42 hard plus 74 mandatory = 116 above 700,
  38 soft; strict exit 1 remains expected debt, not full acceptance.

All four files were checked UTF8 without BOM and remain below 500 effective
lines. This clears one original oversized file: 29/145 (20.0%) structurally
cleared, not 20% of runtime/security/release acceptance. GWP05 source/docs freeze
and Cargo transfer remain unacknowledged; this checkpoint is unreleased.

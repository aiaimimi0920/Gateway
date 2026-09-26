# Browser-pool profile clone ownership checkpoint

Accepted at 2026-09-14 01:06:37.583 UTC. Incremental S18 and whole-plan release
acceptance remain open.

Entry 10,147 -> 10,084 effective lines. Six complete helpers move into a 72-line
profile owner. One factory captures the existing root logger and returns the
three existing clone/removal/cleanup entry points. Bodies change only indentation;
all root launch/retry/context-disposal call sites and error handling remain fixed.
The factory adds no startup I/O, mutable state, browser or context ownership.
Root retains fs/crypto imports that still have unrelated callers.

Three private fixture exports are added before baseline (58 -> 61 lines). Five
new contracts have 82 lines. Paired complete browser-pool suites pass 55/55 with
identical identities/warnings and no skips. Real filesystem tests protect nested
copying, source preservation, skipped directory junctions, cleanup flag-before-
await ordering, repeated cleanup, root/outside/empty refusal and missing-source
errors. One EBUSY copy stub verifies best-effort copying and is restored together
with builtin ESM bindings and logging. It does not prove native sharing-mode locks.

Contained storage fixtures restore environment and remove all created profile
trees; final fixture census is zero. The existing managed-root deletion guard
remains unchanged. Stronger canonical containment, naming-collision avoidance,
bounded recursion/copy size and rollback behavior are outside this structural
checkpoint. Actual busy-browser recovery and concurrent context disposal remain
later lifecycle gates.

The nested package contract grows 158 -> 159 lines solely by adding the profile
module. Paired tests pass 1/1, preserving all original byte/manifest/checksum
assertions. These synthetic packages do not establish a production release or
separate packaged browser/factory execution.

Source/UTF-8-no-BOM/syntax, checker 19/19, ratchet and separate Gateway/Neuro diff
checks pass. All owned native handles are terminal. No Node formatter is configured;
formatting is preserved. The 702-input union preserves 699 neighbors and published
web assets. Independent read-only review found no concrete regression. No Rust,
live service/provider, dependency, checker policy, baseline or exception changed.

Strict: 1,859 scanned, 16 hard, 24 mandatory, 40 soft; 40 above 700. Clearance
remains 105/145 (72.4%). Browser-pool entry and global strict clearance stay open.

Immutable evidence: target/effective-line-evidence/20260914-browser-pool-profile-owners/scope.json.
SHA-256: 6d3d8dd5ca7935425d93eaa0648ce4875a65c8a0ae8b73213d2cc74502cf2add.
Gateway HEAD 4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d: 2,084 entries
(182 modified, one unstaged deletion, two staged deletions, 1,899 untracked).
Neuro HEAD bf818f0324024634bc890585efb78cc8e603d11a: 192 entries
(ten modified, 182 untracked). Census precedes documentation publication.

Next: context creation/reuse/registry/disposal/capacity ownership. Main has read
the complete reuse inspector and five lifecycle functions; pre-extraction tests
must cover registry mutation and cleanup beyond the existing pure capacity tests.
S06 implementation/cursor and final build coordination remain GWP-20260912-01.
Full language/provider/release, runtime-profile governance and packaged runtime/
UI/Docker gates remain open. No persistent deployment occurred; final target
remains persistent 4200 and no persistent 4226.

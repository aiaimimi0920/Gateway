# Lane S: release package ownership

Coordinator scope: tools/package-gateway-release.ps1, its two package-release
owners, source-state contract source locations and nested package coverage.
Rust/Gemini and the shared Cargo window remain owned by S06.

The 701-effective-line entry is now 401. Artifact copying/immutable evidence
and source/build provenance move into cohesive owners of 121 and 181 effective
lines. All 9 extracted functions are unchanged. The complete entry orchestration
is byte-exact after removing those blocks and adding two relative dot-source
lines. Path validation, parameter defaults, build invocation, allowlists,
manifest/checksum generation, publication and rollback remain in the entry.

The owners live under tools/package-release and ship through the existing
recursive tools packaging. This intentionally preserves the packaged entry's
relative imports; placing its owners outside the shipped tree would break that
relationship. Nested package verification includes all three real files and
their support records, checksums and bytes. No policy exclusion or baseline
change was made.

Validation: exact extraction verifier, PowerShell parser (zero errors in all
three files), source-state 6/6, package/layout contracts 22/22, nested package
1/1, ratchet and diff pass. All modified PowerShell owners are UTF-8 without
BOM. Evidence lives in target/effective-line-evidence/package-release, including
before.ps1, verify-extraction.mjs, source-state.log, package-contract.log,
nested-package.log and ratchet.log.

Structural debt clearance reaches 36/145 (24.8%): 109 files above 700 remain.
Inventory: 1198 files, 39 hard / 70 mandatory / 38 soft. This checkpoint is
structural acceptance; inherited source-enumeration, copy and publication race
boundaries still require targeted lifecycle review. Fixture packages used
synthetic binaries in temporary directories, not a new compiled release.
No Cargo/build/live provider ran. GWP-20260908-06 still needs an explicit shared
source/docs freeze receipt and fresh coordinator acceptance before release.

## Evidence destination link admission

Independent owner review identified linked evidence descendants as a concrete
escape from lexical containment. Packaging now rejects reparse-point descendants
between the configured source authority and the evidence file, before build/
publication work. Immutable evidence copy repeats that check before and after
parent creation. The configured authority itself remains trusted; this does not
reject an intentionally mapped workspace or redefine the release root.

A real Windows junction regression covers both the evidence root and version
directory pointing at an outside sentinel directory. Both fail without any
outside write or release creation; cleanup removes only each test-owned junction.
Focused 1/1 (two subcases), package/layout/link 23/23 and nested package 1/1 pass.
PowerShell parsers, ratchet and diff pass; entry/copy owner are 403/144 effective
lines, UTF-8 without BOM. Logs: link-contract.log, hardened-package-contract.log,
link-nested-package.log and link-ratchet.log. The extraction verifier is historical
and predates these intentional behavior changes.

This check does not prevent a concurrent process replacing an ancestor after
validation. Immutable evidence source stability and broader publication resource
review remain open; no blanket race-free guarantee is made. No Cargo/build or
real release ran; all native sessions are terminal and shared transfer is pending.

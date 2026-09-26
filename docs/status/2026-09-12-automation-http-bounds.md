# Credential automation HTTP response bounds

Accepted: 2026-09-12 21:35:10 UTC. Gateway-only hardening checkpoint.

The HTTP driver previously accumulated the whole response before enforcing its
2 MiB output limit. A frozen loopback contract reproduced two delayed rejections:
an oversized declared Content-Length with no arriving body and an oversized
chunked body whose terminator was withheld both exceeded the two-second decision
budget. The original driver timeout was ten seconds in these fixtures.

The HTTP reader now rejects an oversized advertised length before reading and
checks the remaining byte budget for every decoded chunk. It reserves storage
fallibly and returns its bounded byte vector directly. The 2 MiB limit, request
JSON, authentication/header construction, status admission, timeout, parse rules
and error strings are retained. Script execution and its post-read size check
remain unchanged; this checkpoint does not establish script I/O bounds.

All eight frozen contracts pass: the two open-response regressions, exact-limit
JSON without truncation, ordinary chunked JSON, finite excess, malformed JSON,
non-success status without a body read and truncated-body diagnostics. Every case
also checks the request line, content type and complete request JSON. The server
uses only a random loopback port and synthetic data, caps request reads at 16 KiB,
has one deadline-bounded socket task and aborts/joins that task before assertions.
Both regression files remain byte-identical to their pre-fix snapshots.

src/credential_pool_automation/driver.rs grows from 131 to 150 effective lines,
including test wiring. The contract and fixture owners are 110 and 179. Exact
comparison preserves all three function signatures, two unrelated complete
bodies, the byte-limit constant and 295 neighboring inputs. Removing only the
reviewed read change, import and test wiring reconstructs the complete original
owner. No shared helper, dependency, policy, baseline or exception changed.

Fresh serialized gates pass: frozen HTTP contract 8/8; original automation units
10/10 in both phases; cargo check --offline --locked --all-targets; scoped
formatter; checker 19/19; adoption ratchet; exact source/UTF-8/no-BOM proof; and
both repositories' git diff --check. All Cargo gates are terminal. Global
formatting still reports only the two unchanged S06 runtime-mirror files.

Strict scans 1,609 files: 29 hard, 29 mandatory and 40 soft; 58 remain above 700.
Clearance stays 87/145 (60.0%). Immutable evidence is
target/effective-line-evidence/20260912-automation-http-bounds/scope.json.
It binds frozen snapshots, paired test cases/results, compiler/checker logs,
inventories, source proof and separate repository observations. At capture,
Gateway has 1,778 dirty entries: 173 modified, one unstaged deletion, two staged
deletions and 1,602 untracked. Neuro has 192: ten modified and 182 untracked.
Both HEADs and inherited deletions remain unchanged; earlier scopes stay immutable.

Desktop native process/profile ownership is the next structural review. Script
I/O lifetime, archive filesystem races/blocking I/O, scheduler shutdown and
lock/state cardinality remain separate. S06 ownership, GWP-20260912-01,
runtime-profile governance and final freeze/build transfer remain pending. No
immutable release, packaged runtime/UI acceptance or persistent deployment occurred.

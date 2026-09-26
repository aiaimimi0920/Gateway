# Console document and secret ownership

## Scope and starting checkpoint - 2026-09-11

Owner: the parallel coordinator. State: `structural_green`; paired verification
is terminal. Integrated product-release acceptance remains separate.
Exclusive production scope: `src/console/document.rs`, `src/console/document/`,
`src/console/secrets.rs` and `src/console/secrets/`. Existing Console contract
tests may receive focused regression coverage only if a confirmed defect needs
a separate hardening change. S06 Rust/Gemini and its original plan cursor remain
owned by their existing executor.

The official scanner measures the document owner at 1,022 effective lines and
the secret owner at 1,614. The current inventory is 1,312 files, 35 hard, 62
mandatory and 40 soft; 97 files remain above 700. The preceding Console test
decomposition is accepted separately in `console-contracts.md`.

Document extraction keeps public document/diagnostic types, public entry points,
canonical serialization and credential materialization at their current paths.
Private owners separate cross-reference/identity diagnostics, URL validation
and the existing account-group unit tests. The secret owner separates admission,
classification, descriptors, identity mapping, masks, pointers, URL redaction
and candidate validation, preserving keep/replace/clear and public response shapes.

Snapshots are under
`target/effective-line-evidence/20260911-console-document-secrets/`. The paired
proof is the two document unit tests plus the seven existing Console integration
targets (134 tests). Preserve entire moved functions/types, then run the same
gates, scoped formatter, dependent compile check, checker tests and ratchet.
Every completed source owner must remain below 500 effective lines without a new
exception. Record security, resource and performance findings separately from
pure movement.

The GWP-20260908-06 integrated release window remains unacknowledged. Scoped
compilation is serialized after a current process check; no global source/docs
freeze, S06 ownership transfer or immutable release is inferred from that check.

## Verified result - 2026-09-11

Document is now 294 effective lines; diagnostics 355, URL rules 130 and the two
account-group unit tests 254. Public document types and entry points remain in
the original module. The account-group test namespace is still
`console::document::tests`.

Secrets is now 350 effective lines; admission 112, classification 135,
descriptors 146, identity 308, masks 40, pointers 310, URLs 79 and validation 194.
Public wire types and the three document/grant entry points remain in the root;
crate-level key classification and URL redaction keep their original paths via
re-exports. Private helper/type/member visibility is limited to the owning
document or secrets subtree.

The two extraction verifiers compare the complete retained roots and every new
file against controlled transformations of the captured source, allowing only
imports, module registration, necessary visibility changes and formatting. All
function bodies, type definitions, comments and tests are preserved. The 23
existing Console integration-test files also retain their prior source hashes.

Paired baselines and final gates pass 2 document unit tests and all 134 tests in
the seven integration targets. Intermediate document-only verification passed
58/58. The final all-targets check exits 0; no new source warning was added.
The persistence file-symlink test still returns early on Windows error 1314;
its rejection assertion is not executed platform proof.

Scoped formatter, checker 19/19, ratchet, UTF-8/no-BOM checks and Gateway/Neuro
diff checks pass. The common development-standard contract passes. Strict is
still red with 1,323 scanned, 34 hard + 61 mandatory = 95 above 700, and 40 soft.
Both review agents failed to start because their configured model is unsupported;
acceptance uses the coordinator's inspection, source proofs and real gates,
without claiming independent review.

The audit found no introduced state, I/O, lifecycle, allocation site or changed
algorithm. Existing recursive traversal and post-serialization byte admission
are preserved; this structural checkpoint does not establish a new process-memory
bound or complete the broader hardening plan. Full formatting still reports
only the two S06-owned runtime-mirror files; no integrated release is included.

Detailed per-file counts, commands and evidence are in
[the acceptance report](../../status/2026-09-11-console-document-secrets.md).

# Console runtime and HTTP ownership

## Starting checkpoint - 2026-09-11

Owner: the parallel coordinator. State: `structural_green`; paired gates are terminal.
Scope: `src/console/runtime.rs`, `src/console/runtime/`,
`src/http/routes/internal_console.rs` and `src/http/routes/internal_console/`.
The Gemini session service and the existing S06 Rust/Gemini lane remain outside
this scope. Gemini handler bodies stay in the existing HTTP root.

The scanner measures runtime at 1,087 effective lines and HTTP handlers at 822.
The current inventory has 1,336 files, 33 hard, 60 mandatory and 40 soft;
93 files remain above 700. Snapshots are under
`target/effective-line-evidence/20260911-console-runtime-http/`.

Runtime extraction preserves complete async methods, writer-guard lifetimes,
await positions and commit/recovery ordering. Private owners separate commit,
activation outcomes, startup recovery and snapshot/revision reconciliation.
Public runtime types and backend contracts retain their original paths.

HTTP extraction separates session, credential/provider probe, route mutation
and revision handlers. Request/response types and shared authentication/context
helpers remain at the original paths; handler re-exports keep router wiring.
Authentication, secret-grant checks, no-store/ETag headers and error mappings
must remain unchanged.

The paired proof uses the two persistence-lock tests plus nine Console
integration targets: the prior 134 cases, 13 hot-reload cases and five session
cases. Final gates include source equivalence, all-targets compilation, scoped
formatter, checker tests, ratchet and independent-repository Git checks.
Every new or completed owner must stay below 500 effective lines.

Cargo gates are serialized after a live process check. The GWP-20260908-06
integrated source/docs freeze and release transfer are still pending; no
S06 ownership or product-release acceptance is inferred from this batch.

## Verified result - 2026-09-11

Runtime is now 334 effective lines, with activation 128, commit 249, recovery 235,
revision-state projection 95 and snapshot installation 112. Complete async
methods moved together; no await, writer-guard scope or state ownership changed.
Public runtime types remain in the root. Necessary relative type references in
moved methods were qualified to the same `crate::console` types.

The HTTP root is 314 effective lines; probes 173, revisions 146, route-config
handlers 155 and session handlers 104. Thirteen public handlers are re-exported
at their existing router paths. Wire types, common request/response helpers and
the three byte-identical Gemini handler bodies remain in the root. No Gemini
service source or router registration was changed.

Full controlled-source comparison passes for both roots and all nine children.
The 51 previously recorded integration-test, document/secret and persistence/
journal source hashes remain unchanged. Paired baselines and final runs pass
2 lock tests plus 152 integration cases, including 13 hot-reload and five session
cases. The inherited file-symlink guard still bypasses its rejection assertion
on Windows error 1314; that assertion is not executed platform proof.

Intermediate runtime library compilation and final all-targets compilation pass.
Scoped formatter, checker 19/19, ratchet, UTF-8/no-BOM, Gateway/Neuro whitespace
checks and the development-standard contract pass. No new warning was introduced.
Strict remains red at 1,345 scanned, 33 hard + 58 mandatory = 91 above 700 and
40 soft. Structural progress is 54/145 (37.2%).

This is structural acceptance. No new state, resource lifecycle, allocation site
or algorithm was introduced, and no additional hardening or independent-review
claim is made. The configured default review model remains unavailable from the
preceding attempts. The two S06 formatter differences and release transfer stay
pending. Details and commands:
[acceptance report](../../status/2026-09-11-console-runtime-http.md).

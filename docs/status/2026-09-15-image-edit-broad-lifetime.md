# Image-edit broad capture and context lifetime repair

Accepted 2026-09-15T06:12:28.172Z. Capture owner grows 283 -> 317 effective lines
(+34); root 390 -> 397 (+7). Page owner remains unchanged at 155. All stay below 500.

The broad probe previously retained capture listeners/pending work and only closed
its browser context on successful execution. Exceptions skipped context cleanup,
and late upload callbacks could write binary artifacts after the probe ended.

Capture now owns the exact page/CDP/socket listener references. stop is idempotent,
invalidates before detach, clears the CDP index, attempts every detach even when
one fails and releases the registry. Queued handlers reject payload access after
stop; six asynchronous read families guard publication/file work after suspension.
Rejected header reads check stop before calling fallback header accessors.

Root post-launch execution is inside finally cleanup: capture stops before context
closure, cleanup errors are suppressed and original outcomes survive. Launch stays
outside try. The complete business body is unchanged apart from indentation.

Fresh baseline 480/480 passes. Thirty-one added lifetime cases pass 2/31 before
the fix and fail 29/31 for real late writes/state, stale access, listener retention
or missing context closure. The two existing-safe cases are rejected CDP response
reads and context launch failure. Final 511/511 retains all 480 previous identities
and all 31 additions, with no skips. One old close-count assertion shared by three
failure tests intentionally changes 0 -> 1; all other existing assertions remain.

Tests execute actual owner/root code with deferred Promises and owned EventEmitter
listeners. They cover completion/rejection at all six capture read boundaries,
inherited/dynamic socket listeners, repeated stop, partial detach failure, queued
payloads, post-stop attachment, startup/output failures, error identity and real
temporary binary-file isolation after success/rejection. Fixture bytes are unchanged.

Independent review confirmed the cleanup/guard boundaries. A suggested guard
before assigning response text to a private unpublished event was not adopted:
the plain local assignment cannot publish shared state or access payload properties;
the stopped guard runs before pushEvent. The contract suppresses observable late
work, not harmless local continuation or already-started browser reads.

Source/inverse and exact root-body preservation, UTF-8/no-BOM, syntax, checker 19/19,
ratchet and Gateway/Neuro Git checks pass. No Node formatter is configured.
Gates are serialized with frozen inputs and terminal; temporary modules/roots empty.
Nested-worker package 1/1 passes before/after unchanged. This manual probe and owners
are outside that package contract; no release/runtime inclusion is inferred.
Prior pool 932/932 and exporter 25/25 remain retained evidence, not fresh reruns.

Evidence: target/effective-line-evidence/20260915-image-edit-broad-lifetime/scope.json.
SHA-256: 6505ddf1aaa14897ddf239e48d3d0c436b7eb7851e1b6f4191fde3b0f7d80b54.
Predecessor: 24d01372980318ea171d48bac16fea4d81406fe9826b841295d7d19e5b8d208b.
Union 894; unchanged neighbors 891. Strict exits 1: 2043 scanned, 13 hard, 22 mandatory,
40 soft; 35 above 700. Clearance 110/145 (75.9%) remains unchanged.

Pending reads are not cancelled or drained. A failed detach may leave an inert
handler attached; cleanup still attempts other handlers and context closure.
Active CDP-map/registry size, raw diagnostics and hostname filters remain follow-up.
Process termination, hung provider calls, native browser/profile/UI and packaged
runtime/Docker/release behavior remain unverified. Preserve acceptedTestFiles (511)
and browserPoolTestFiles (932) for further work.

Continue remaining strict inventory (22 Rust, one CSS, twelve C-like entries),
then the outstanding language/provider/profile/runtime/UI/Docker/release gates.
Full S18 remains open. S06/final-build stays GWP-20260912-01. Persistent 4200,
no persistent 4226; browser defaults 42321/42322. No Rust, credentials, dependency,
checker policy, baseline, sibling source, live runtime, release, deployment or commit changes.

Acceptance Git: Gateway HEAD 4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d, 186 modified,
1 unstaged deletion, 2 staged deletions, 2227 untracked. Neuro HEAD
bf818f0324024634bc890585efb78cc8e603d11a, 10 modified, 182 untracked.
Final documentation hashes and staged/unstaged Git receipts are in publication.json.

# Image-edit broad capture and page ownership

Accepted 2026-09-15T05:51:31.732Z. The standalone broad image-edit runner decreases
796 -> 390 effective lines (-406). Capture owner is 283; page owner is 155.
Every new/extracted source and test is below 500. This clears one >700 entry.

Capture state now belongs to one factory: event retention, first-marker transport,
request/response timestamps, secondary signaler progress, CDP request metadata and
binary finalize upload capture. Eight live getters retain asynchronous state reads;
listeners attach after the existing Network.enable call and before navigation.
The page owner retains the serialized DOM and blob callbacks, followed by the
original MIME/label/SHA-256/file export policy.

Profile/env setup, reference-image copy/fallback, upload chooser, UI actions,
transport/image waits, style selection, success classification, result schemas,
storageState and the final context.close remain in the entry. Source projection
reconstructs the complete original root from thirty exact operations and rebuilds
both prepared owner files from the original blocks. No feature fix is mixed in.

Paired Node tests pass 480/480 with identical identities/warnings and no skips:
all 446 previous identities plus 34 new cases (14 capture, 7 page, 13 execution).
Original inline bodies run before integration; actual extracted bodies run after.
Tests protect event retention/marker identity, filter and header fallbacks, binary
and Latin1 upload captures, CDP body decoding/bounds, websocket phases, secondary
poll timestamps, DOM filtering/schema, 70001-byte blob export, MIME/label choices,
actual-root startup/polling/output ordering, preview distinction and error paths.

Native imports establish loadability and empty-input behavior. Detailed behavior
is tested through controlled source execution; serialized page callbacks run in
isolated VMs. Artifact fields and stdout keys are checked, but every stdout value
is not compared with its artifact counterpart. Complete root reconstruction and
static review separately preserve current result wiring. Real browser/provider
and runtime behavior are not established by these tests.

Independent source review found no introduced defect. Test review identified the
above coverage limits; no source change was warranted. No Node formatter is
configured. Source/inverse, UTF-8/no-BOM, syntax, checker 19/19, ratchet and both
Gateway/Neuro Git checks pass. Gates are serialized and all processes terminal;
temporary probe roots and testable modules are empty.

Existing nested-worker package test passes 1/1 before/after with unchanged bytes.
This manual diagnostic runner has no repository callers or production package
entry; the package contract does not include this runner or its two new owners.
No packaged-runtime or release inclusion is inferred from that unrelated contract.
Prior browser-pool 932/932 and exporter 25/25 remain retained, not fresh reruns.

Evidence: target/effective-line-evidence/20260915-image-edit-broad-owners/scope.json.
SHA-256: 24d01372980318ea171d48bac16fea4d81406fe9826b841295d7d19e5b8d208b.
Predecessor: afb961d25c16058792df47054266f1d42a91c812a554176deba45eddc7e7dbb6.
Union 892; unchanged neighbors 891 after preparation; web assets and preceding
worker/program-handle source/test hashes are frozen.
Strict exits 1: 2041 scanned, 13 hard, 22 mandatory, 40 soft. There are now
35 files above 700; clearance 110/145 (75.9%).

Remaining strict inventory comprises 22 Rust files, one CSS file and twelve
C-like files. Runtime-profile/vendor findings remain separate. The broad runner
still inherits missing failure-path context cleanup/capture stop, unbounded CDP
request retention, raw diagnostic data and substring hostname filtering. These
are explicit follow-up boundaries, not guarantees added by this structural move.
Preserve all 480 accepted Node identities and 932 browser-pool identities.

Full S18/strict/language/provider/profile/packaged runtime/UI/Docker/release stays
open. S06/final-build remains GWP-20260912-01. Persistent target 4200; no persistent
4226; browser defaults 42321/42322. No Rust, credentials, dependency, checker
policy, baseline, sibling source, live runtime, release, deployment or commit changes.

At acceptance, Gateway HEAD 4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d has 186
modified, 1 unstaged deletion, 2 staged deletions and 2223 untracked files.
Neuro HEAD bf818f0324024634bc890585efb78cc8e603d11a has 10 modified and 182
untracked files. Final documentation/Git receipts are in publication.json.

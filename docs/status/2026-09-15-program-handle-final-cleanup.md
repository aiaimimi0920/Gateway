# Standalone execution-final capture cleanup

Accepted 2026-09-15T05:21:33.929Z. The execution owner grows 436 -> 441 effective
lines (+5) and stays below 500. The standalone probe remains unchanged at 496.

Execution previously closed the context without stopping its current capture.
Owned callbacks remained active after success or rejection, including the last
page after adoption; deferred text and Cookie completion could still mutate state.

Capture ownership now starts as null after successful context launch, is assigned
only after acquisition, and is stopped in finally before context.close. Stop errors
are suppressed so context closure still runs and successful completion or the
original business error survives. Launch-before-try and adoption ordering remain.

Fresh baseline: all 433 prior tests pass. The thirteen added exit-boundary tests
pass 3/13 before the repair: all ten listener/stop-attempt/deferred-publication
regressions fail, while launch/pages/capture failures correctly own no capture.
Final combined suite passes 446/446 with no skips, retaining all 433 identities
and all thirteen new identities. No fixture was altered for this checkpoint.

Four existing assertions intentionally change: final stdout/stop/close ordering,
popup listener count 1 -> 0, adoption stop count 1 -> 2, and final popup stop in
the exact capture/adoption sequence. All other existing assertions and test names
remain. Projection records these substitutions and reconstructs the prior source
and test bytes; this is not a claim of identical baseline/final assertions.

New tests execute the actual execution body and native standalone capture with
EventEmitter pages, deferred Promises and real temporary output files. They verify
success/rejection, adoption, inherited listeners, stop-before-close, detach failure,
original error identity, late text/Cookie isolation and absent-owner startup.
Independent read-only review found no introduced defect.

Package contract is unchanged and passes 1/1 before/after. Source inverse,
UTF-8/no-BOM, syntax, checker 19/19, ratchet and Gateway/Neuro Git checks pass.
No Node formatter is configured. Gates are serial and inputs frozen; gate processes
are terminal and temporary modules/output roots are empty. Prior browser-pool
932/932 and exporter 25/25 remain retained evidence, not fresh reruns.

Evidence: target/effective-line-evidence/20260915-program-handle-final-cleanup/scope.json.
SHA-256: afb961d25c16058792df47054266f1d42a91c812a554176deba45eddc7e7dbb6.
Predecessor: 839036e189caf4548100470c132ad15dd27e308aef14f73eb2a56d7986e80865.
Union 885; unchanged neighbors 883; standalone root, execution fixture, shared
dependencies, package contract and web assets remain frozen.
Strict exits 1: 2035 scanned, 13 hard, 23 mandatory, 40 soft; 36 above 700.
Clearance remains 109/145 (75.2%).

Native stop marks capture inactive before detach. If page.off throws, successful
listener removal is not guaranteed, but final context closure is still attempted.
Already-started browser reads are not cancelled. Failed replacement acquisition
can repeat stop on the old capture; native stop remains idempotent.
This checkpoint does not establish real browser/provider/profile/runtime/UI,
Docker or release success.

Preserve scope.acceptedTestFiles (446), including lifetime/exit regressions, and
browserPoolTestFiles (932). Next: remaining strict debt, including the image-edit
broad runner at 796, and the outstanding language/provider/runtime/release gates.
Full S18 remains open. S06/final-build stays GWP-20260912-01. Persistent target
4200; no persistent 4226; browser defaults 42321/42322.

At acceptance, Gateway HEAD remains 4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d
with 185 modified, 1 unstaged deletion, 2 staged deletions and 2215 untracked
files. Neuro HEAD remains bf818f0324024634bc890585efb78cc8e603d11a with 10
modified and 182 untracked files. Post-publication Git counts and both staged/
unstaged checks are recorded separately in publication.json.
No Rust, credentials, dependency, checker policy, baseline, sibling source,
persistent runtime, release payload, deployment or commit changes.

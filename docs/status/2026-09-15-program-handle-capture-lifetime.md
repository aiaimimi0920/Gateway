# Standalone capture stop lifetime repair

Accepted 2026-09-15T05:01:44.798Z. The program-handle probe grows 482 -> 496
effective lines (+14), remaining below the 500-line owner limit. The added lines
guard an existing per-capture lifecycle; no extraction or unrelated refactor.

Removing event listeners left queued handlers and pending response/cookie work
able to publish after stop. During page adoption, the next capture receives the
same state, allowing old-page results to overwrite current evidence.

The repair adds one stopped flag per capture, checks all three callbacks before
payload access, checks again after response.text completion/rejection, and prevents
late cookie publication. stop sets the flag before detaching its own listeners.
New captures remain active with the same accumulated state; external listeners
and repeated-stop behavior remain intact.

Before production edits, 422 preservation tests passed and all 11 stop regressions
failed. Afterward, 433/433 pass: all 409 prior identities, thirteen active-capture
cases and all eleven failing-before regressions. Tests execute native standalone
listeners on EventEmitter pages with deferred Promises. They cover queued payload
access, response/media text completion and rejection, cookie completion, adopted
state isolation, listener ownership, active async work, previews and media URLs.
Independent static review found no introduced defect.

Package contract remains unchanged and passes 1/1 before/after. Source inverse,
UTF-8/no-BOM, syntax, checker 19/19, ratchet and Gateway/Neuro Git checks pass.
No Node formatter is configured. Gates are serial with frozen inputs and terminal;
temporary modules and output roots are empty. Shared browser-pool sources are
unchanged; prior pool 932/932 and exporter 25/25 are not rerun results.

Evidence: target/effective-line-evidence/20260915-program-handle-capture-lifetime/scope.json.
SHA-256: 839036e189caf4548100470c132ad15dd27e308aef14f73eb2a56d7986e80865.
Predecessor: bd70d9eda9ff5df12fcbfb97afc3a7d5bd689a0182feb362a50497b2b3e770c1.
Union 884; unchanged neighbors 883; web assets frozen.
Strict exits 1: 2034 scanned, 13 hard, 23 mandatory, 40 soft; 36 above 700.
Clearance remains 109/145 (75.2%). Program-handle root 496; execution owner 436.

This repair applies after explicit capture.stop, including page adoption. It
suppresses late publication without cancelling already-started browser reads.
The execution owner's final context.close boundary is unchanged and still has
no explicit final capture.stop. Real browser/provider/profile/runtime/UI/release
and snapshot/UI event boundaries remain separate work.

Preserve scope.acceptedTestFiles (433), including network-stop regressions, and
browserPoolTestFiles (932). Remaining strict inventory includes image-edit broad
runner 796. S06/final-build remains GWP-20260912-01; full S18/strict/language/
provider/runtime-profile/packaged runtime/UI/Docker/release remain open.
Persistent target 4200; no persistent 4226; browser ports 42321/42322.
No Rust, credentials, dependencies, checker policy, baseline, sibling source,
persistent runtime, release, deployment or commit changes. Inherited dirty work remains.

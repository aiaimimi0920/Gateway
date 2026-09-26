# Browser-pool cookie restoration ownership checkpoint

Accepted at 2026-09-14 03:24:08.657 UTC. Incremental S18 and whole-plan release
acceptance remain open.

Entry 9,382 -> 9,143 effective lines. Eight header/auth/storage-state functions
move to a 247-line owner with indentation-only body changes. One factory captures
the root logger before app-factory initialization. Fetch authentication keeps its
parser binding; exact reverse projection preserves other entry bytes, public
exports and call sites. The owner adds no registry, startup I/O or reverse import.

Pre-extraction fixture exports grow 69 -> 71 lines. New boundary/real-browser
test files have 86/43 lines: 11 contracts verify filtering/final header values,
auth detection, partial recovery, all-failed original errors, redacted logs,
invalid/malformed sources and source preservation. Three use real offline browser
cookie jars and localStorage, including empty cookies and ignored stored origins.
Existing URL-cookie and app-recovery tests remain intact. Tests use synthetic
data, isolated contexts, blocked service workers and locally fulfilled HTML.

Paired complete Node suites pass 99/99, identical identities/warnings, no skips,
using test-concurrency=1. Package contracts pass 1/1; their only change adds the
cookie owner to the existing byte/manifest/checksum assertions (161 -> 162 lines).
Source/encoding/syntax, checker 19/19, ratchet and both repository diff checks
pass. All gates are terminal; temporary storage/testable modules are cleaned up.
No Node formatter is configured. No provider or production package acceptance
is claimed from these synthetic contracts.

Independent test review found no blocker. Initial projection review could not
locate ignored evidence; a later direct-path review confirmed body preservation
and bindings. Exact source proof preserves 713 neighbors in a 716-input union,
including published web assets. No Rust, dependencies, checker/adoption policy,
exceptions, release artifacts or sibling sources changed.

Strict: 1,873 scanned, 16 hard, 24 mandatory, 40 soft; 40 above 700. Clearance
remains 105/145 (72.4%). The shrinking browser-pool entry still requires splitting.
Evidence: target/effective-line-evidence/20260914-browser-pool-cookie-owners/scope.json.
SHA-256: 9eb04a18ef267cdf3aa459854c4262951716246cddd9818ffee82182da09c7ae.
Gateway HEAD 4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d: 2,107 entries
(182 modified, one unstaged deletion, two staged deletions, 1,922 untracked).
Neuro HEAD bf818f0324024634bc890585efb78cc8e603d11a: 192 entries
(ten modified, 182 untracked). Census precedes documentation publication.

Next: concrete program/share navigation and share-entry following; keep page
adoption, old-page cleanup and network capture with existing callers. S06 scope/
cursor and final build transfer remain reserved under GWP-20260912-01. Full strict,
language/provider/release and packaged runtime/UI/Docker gates remain open. No
deployment occurred; final target is persistent 4200 with no persistent 4226.

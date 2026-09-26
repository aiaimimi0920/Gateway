# Browser-pool executable and TLS ownership checkpoint

Accepted at 2026-09-14 00:28:52.651 UTC. Incremental S18 and whole-plan release
acceptance remain open.

Entry 10,424 -> 10,333 effective lines. The executable owner has 45 lines and
the TLS owner 53. Exact projection preserves five function bodies, three platform
path arrays and every remaining root byte except the recorded imports. The root
retains launch options, contexts, pages, server construction and shutdown. New
owners add no startup I/O or mutable module state. Original synchronous TLS I/O,
SAN/options, return fields, path resolution and error ordering are preserved.

Three private fixture exports are added before the baseline (54 -> 57 lines).
Five new contracts have 87 lines; the same complete browser-pool suite passes
45/45 before and after, with identical test identities and warnings and no skips.
Tests use actual temporary files and generated certificates, checking override
precedence, boolean fallback identity, X509/private-key matching, SANs, stored PEM
hashes, reuse, incomplete-pair replacement, relative root, slug and directory
errors. Generated keys are not printed. Environment values are restored. All
owned temporary roots are canonically contained and removed; final census is zero.

The nested package contract grows 155 -> 157 lines solely by adding two module
paths. Paired tests pass 1/1. Original assertions are unchanged and protect copied
bytes, manifest records, checksums and lengths. These two modules are not separately
imported from that temporary package; source import/reexport resolution is exercised
by Node tests. Synthetic package binaries do not prove a production release.

Source/UTF-8-no-BOM/syntax checks, checker 19/19, ratchet and separate Gateway/Neuro
diff checks pass. No Node formatter is configured; original formatting and hygiene
remain intact. All owned gate sessions are terminal. Independent read-only review
found no concrete regression. The 696-input union preserves 692 frozen neighbors
and published web assets. No Rust/compiler, browser/provider, live service,
dependency, checker policy, baseline or exception changes occurred.

Strict: 1,853 scanned, 16 hard, 24 mandatory, 40 soft; 40 above 700. Clearance
remains 105/145 (72.4%). The residual browser-pool entry is still debt. The preceding
Rust acceptance's two reserved S06 formatting failures remain frozen unchanged;
this Node-only batch did not repeat the global Rust formatter.

Immutable evidence: target/effective-line-evidence/20260914-browser-pool-config-owners/scope.json.
SHA-256: d61f43870ef049079978bc396c7ff70af2a294dc998d548f272560ba7bb816c5.
Gateway HEAD 4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d: 2,074 entries
(182 modified, one unstaged deletion, two staged deletions, 1,889 untracked).
Neuro HEAD bf818f0324024634bc890585efb78cc8e603d11a: 192 entries
(ten modified, 182 untracked). Census precedes documentation publication.

Next inspect profile/storage configuration and runtime-state ownership. No
cross-platform browser execution, TLS network handshake, certificate permission/
expiry hardening or live lifecycle validation is claimed. S06 implementation/cursor
and final build coordination remain reserved under GWP-20260912-01. Full strict,
language/provider/release, runtime-profile governance and packaged runtime/UI/Docker
gates remain open. No production release or deployment occurred; final target
remains persistent 4200 and no persistent 4226.

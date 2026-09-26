# Implementation-line ownership

Owner: parallel coordinator. State: accepted. Started: 2026-09-13 UTC.
Accepted: 2026-09-13 01:29:48 UTC.

Scope: src/implementation_lines.rs and private implementation_lines/ owners.
The original entry was 1,585 effective lines. Preserve the public feature constants,
line enum/variants/methods, profile aliases, adapter-first payload inference,
compile checks, error status/code/text and unknown-profile behavior.

Keep feature metadata together. Extract profile/adapter lookup and payload
inference as separate pure owners, while the entry retains compile enforcement
and stable exports. Split the existing tests into fixture, profile mapping,
payload mapping, generated feature-profile contracts and explicit compile checks.
Preserve every test body, cfg attribute and macro invocation. Do not change Cargo
features, provider routing or policy while moving these definitions.

The current call sites include management account validation, route-proof
classification, keepalive, provider quota/folder synchronization and disabled
provider stubs. Preserve those callers, manifests and the protocol-registry
contracts byte-for-byte. Require paired default and no-default-feature line
tests, paired default official-profile tests, fresh all-targets, scoped formatter,
checker 19/19, ratchet/strict inventories, source/encoding proof and both Git checks.
All extracted owners must remain at most 500 effective lines.

Review URL classification and allocation separately after structural proof. This
module owns pure catalog/validation logic, with no process, connection or task
lifecycle to transfer. Serialize every Cargo gate and do not edit Rust while it
runs. S06 keeps its existing implementation/cursor; GWP-20260912-01, runtime-profile
governance and final freeze/build transfer remain pending. No dependency, baseline,
exception, checker-policy, release or persistent runtime change belongs here.

Evidence: target/effective-line-evidence/20260913-implementation-line-owners/.

The entry is now 114 effective lines; all ten source/test owners are at most 325.
Exact proof preserves 65 production definitions, 64 public paths, three enum
methods, 31 explicit test bodies and fourteen macro invocations, with 361 unchanged
neighbors. Paired default line/profile gates pass 37/37 and 7/7; paired disabled
line gates pass 41/41. Both feature phases cover all 59 distinct test identities.
Fresh all-targets, scoped formatting, checker 19/19, ratchet, source/encoding and
cleanup proof and both Git checks pass. All Cargo gates are terminal.

Strict: 1,635 scanned, 28 hard, 27 mandatory, 40 soft; 55 above 700. Clearance:
90/145 (62.1%). Global formatting retains only the two unchanged S06 findings.
scope.json SHA-256:
90e70606820d2ce8e1dc3cced1d71fd1997e569c76c72355e3fcdfdbd51b2e01.
[Acceptance report](../../status/2026-09-13-implementation-line-owners.md).
Preset ownership is the next structural review; wider completion gates remain open.

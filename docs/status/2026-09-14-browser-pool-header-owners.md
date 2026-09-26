# Browser-pool header sanitization checkpoint

Accepted at 2026-09-14 06:05:36.563 UTC. Entry 7,560 -> 7,507 effective lines;
two pure sanitizers move to a 55-line owner with export modifiers only. Import
the existing object normalizer; preserve both distinct header policies, callers,
spelling, values and own-key enumeration. No new I/O, state or lifecycle.

Fixture grows 93 -> 95; a 40-line test adds five contracts for invalid inputs,
proxy/browser policy differences, immutable inputs, value identity, case-distinct
keys and prototype-shaped own fields. Package contract grows 172 -> 173 with
the single owner path. Paired serialized Node 219/219, identical identities and
warnings, no skips; package 1/1. Source/encoding/syntax, checker 19/19, ratchet
and both Git checks pass. All gates terminal. No Node formatter configured.

Evidence: target/effective-line-evidence/20260914-browser-pool-header-owners/scope.json.
SHA-256: 287b3b4a673d16a9cf823161df2acb7791b1832d44a265bdc31797568c111452.
Union 740; unchanged neighbors 737; web assets preserved. Strict: 1,897 scanned;
16 hard, 24 mandatory, 40 soft; 40 above 700. Clearance 105/145 (72.4%).
Gateway HEAD 4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d: 182 modified, one
unstaged deletion, two staged deletions, 1,966 untracked before docs publication.
Neuro HEAD bf818f0324024634bc890585efb78cc8e603d11a: ten modified, 182 untracked.

Next: preview opening/frame stamping, excluding loopback registries, page leasing
and new-chat controls. The frame probe rejection timer is an observed lifecycle
lead requiring separate reproduction; no behavior change is included here. S06
scope/cursor and final build transfer stay reserved under GWP-20260912-01. Full
strict/language/provider/runtime-profile/packaged runtime/UI/Docker/release gates
remain open. Persistent target stays 4200, no persistent 4226; no deployment.

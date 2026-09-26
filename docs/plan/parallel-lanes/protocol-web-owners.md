# Qwen Web and Xfyun protocol owners

Owner: parallel coordinator. State: accepted structural checkpoint.
Started: 2026-09-12.
Accepted: 2026-09-12 06:15:01 UTC.

Reserved entries: src/protocol/qwen/web_reverse.rs and
src/protocol/xfyun_websocket.rs, plus their new private child owners.
The fresh preceding inventory measures 776 and 963 effective lines respectively.
Both complete entries have been read before designing the boundaries.
Paired default-feature baseline/final tests pass Qwen 12/12 and Xfyun 7/7. Entries
measure 26/28; all 16 files are at most 215 effective lines. Exact production/test
proof and 180 neighboring hashes pass. All native Cargo handles are terminal.

Qwen owners separate request packing, HTTP challenge/session classification,
response accumulation, response-value parsing, response-shape diagnostics and SSE
translation state. The public constants and original namespace stay at the entry.
Xfyun owners separate request packing, payload fields, URL signing, transport and
nonstream execution, frame parsing/rendering and streaming state. Shared private
records and public entry paths remain in the original namespace.

This batch preserves all function bodies, types, aliases, constants, request and
signature construction, response/error handling, token accounting, frame order,
terminal behavior and existing tests. Only required family-local visibility and
imports/module wiring may change. Original full test paths remain stable.
The Qwen feature selection, disabled surface, public compatibility export,
external callers, dependencies and checker policy remain unchanged.

Fresh all-targets, scoped formatter, checker 19/19, ratchet, exact source/test and
neighbor proof, UTF-8 without BOM and both Git checks pass. Strict scans 1,543
files: 31 hard, 33 mandatory and 40 soft; 64 remain above 700. Clearance is
81/145 (55.9%). Global formatting still reports only the unchanged S06 mirror pair.

Review findings awaiting separate reproduction: eager usage-sum overflow in both
protocols, Qwen pending-line admission and diagnostic path bounds, and Xfyun
transport/accumulation resource limits. Preserve them during structural extraction.
No live provider or release acceptance is implied by this local test batch.

Evidence: target/effective-line-evidence/20260912-protocol-web-owners/.
[Acceptance report](../../status/2026-09-12-protocol-web-owners.md).
S06 ownership and pending GWP-20260912-01 freeze/build coordination are unchanged.

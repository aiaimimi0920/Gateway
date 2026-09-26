# Qwen Web and Xfyun protocol ownership

Accepted structural checkpoint: 2026-09-12 06:15:01 UTC.
Owner: parallel coordinator. The entire Gateway plan remains in progress.

The Qwen Web entry decreased from 776 to 26 effective lines and the Xfyun
WebSocket entry from 963 to 28. Both original public namespaces remain intact.
All 16 scoped files are at most 215 effective lines.

## Accepted structure

| File | Effective lines |
| --- | ---: |
| src/protocol/qwen/web_reverse.rs | 26 |
| qwen/web_reverse/request.rs | 138 |
| qwen/web_reverse/http_errors.rs | 76 |
| qwen/web_reverse/accumulation.rs | 105 |
| qwen/web_reverse/response_value.rs | 85 |
| qwen/web_reverse/diagnostics.rs | 54 |
| qwen/web_reverse/stream.rs | 124 |
| qwen/web_reverse/tests.rs | 200 |
| src/protocol/xfyun_websocket.rs | 28 |
| xfyun_websocket/request.rs | 172 |
| xfyun_websocket/payload_fields.rs | 65 |
| xfyun_websocket/signing.rs | 116 |
| xfyun_websocket/connection.rs | 128 |
| xfyun_websocket/frames.rs | 184 |
| xfyun_websocket/stream.rs | 113 |
| xfyun_websocket/tests.rs | 215 |

## Preservation and review

Exact comparison preserves 48 functions, three structs, three aliases, eleven
constants and 25 public entry paths. Thirteen private helpers gain only the
family-local pub(super) visibility required by real callers and original tests.
Shared private Xfyun records retain their existing fields and visibility.
All 19 original tests, six fixtures and complete test module paths are preserved.

Qwen request packing, UUID/time construction, feature flags, multi-turn rendering,
HTTP challenge/session classification, response fallback and thinking-token
filtering remain unchanged. The response parser and SSE translator keep their
existing ordering and terminal behavior. Existing Chinese fixture strings remain
unchanged UTF-8. The multiline browser header constant is preserved exactly.

Xfyun payload precedence, message rendering, signature inputs, query encoding,
TLS root loading, connection/send sequencing, frame classification, usage aliases,
SSE ordering and nonstream aggregation remain unchanged. Extraction retains all
three stream/cryptographic aliases and the shared private frame record.

All 180 neighboring inputs remain unchanged, including feature selection, the
Qwen disabled surface and compatibility export, actual upstream callers,
canonical/SSE modules, dependencies and checker policy. No new forwarding wrapper
or dependency was introduced.

Separate review boundaries remain: both usage parsers eagerly add untrusted u64
counters, Qwen pending-line admission and diagnostic path limits, and Xfyun
transport/accumulation resource bounds. This structural checkpoint preserves
those behaviors. The next regression targets usage fallback overflow and explicit
reported-total precedence.

## Fresh verification

- Paired default-feature Qwen tests: 12/12; Xfyun tests: 7/7, with exact test paths.
- Separate cargo check --offline --locked --all-targets: passed.
- Scoped formatter, checker 19/19, ratchet, complete source/test/public-path proof,
  neighbor hashes, UTF-8 without BOM and whitespace checks: passed.
- Gateway and Neuro git diff --check: passed independently.

All native gates are terminal. These local tests do not establish live provider
or TLS deployment acceptance. Global formatting still reports only the two
unchanged S06 runtime-mirror files.

Strict scans 1,543 files: 31 hard, 33 mandatory and 40 soft. There are 64 files
above 700; accepted clearance is 81/145 (55.9%). Strict still exits 1.

The snapshot records Gateway HEAD 4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d with
1,690 dirty entries: 169 modified, one unstaged deletion, two staged deletions and
1,518 untracked. Neuro HEAD is bf818f0324024634bc890585efb78cc8e603d11a with
192 entries: ten modified and 182 untracked. Counts precede this report.

## Evidence and continuation

Immutable acceptance: target/effective-line-evidence/20260912-protocol-web-owners/scope.json.
The directory also contains the original-source snapshot, bounded projection and
proof scripts, paired Cargo logs and non-Cargo receipts.
[Lane](../plan/parallel-lanes/protocol-web-owners.md).

Earlier accepted scopes stay immutable. S06 retains its implementation/original
cursor; GWP-20260912-01 and the explicit source/docs freeze and release-build
transfer remain pending. No release was built, live service changed or
runtime-profile policy migrated.

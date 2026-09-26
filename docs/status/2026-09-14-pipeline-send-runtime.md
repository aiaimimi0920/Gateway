# Pipeline send runtime baseline

Accepted at 2026-09-13 20:49:33.353 UTC (2026-09-14 local time). This accepts
runtime regression preparation; stage-send structural ownership and the whole
Gateway optimization/release plan remain open.

Seven new integration tests invoke stage_send::run with populated candidates and
real ephemeral loopback HTTP/SSE servers. The fresh locked offline test run
passed 7/7 in 1.68 seconds:

- Buffered response content, caller-visible model, upstream model, usage and
  released permit; the fixture also checks the actual path, body and test auth.
- Three transient-failure attempts followed by a successful second provider,
  with four admissions, first-contact ordering and per-provider AIMD feedback.
- Terminal HTTP 400 without retry or contact with the second candidate.
- Cancellation while queued behind ten occupied permits, with no admission or
  outbound request and no leaked permit.
- Streaming success deferred until EOF, exactly one observable AIMD success
  even after Drop, and no held permit when the stream is returned.
- Returned-stream Drop before EOF produces failure feedback without early success.
- Failed stream startup falls back before returning the successful SSE stream.

The five new files have 9, 60, 234, 158 and 125 effective lines respectively.
They reuse tests/support/mod.rs unchanged. No production Rust changed: stage_send
remains 3,516 effective / 3,792 physical lines, SHA-256
a7cf501c2295aa9ddbb8f0d06fd169890b2c6ee842b70a81536f06402a011e4c.
All 649 frozen inputs, existing tool_stream children and published web assets
remain byte-identical; the accepted input union is 654.

Fixtures use no PostgreSQL, session, route policy, credential row or keepalive.
The lazy Redis pool points to 127.0.0.1:1, while actual upstream listeners bind
127.0.0.1:0. Test state is confined to uniquely created console directories.
Each server requests bounded graceful shutdown and is joined; Drop provides
failure-path abort. Tests wait for detached finalization tasks to release their
AppState captures before cleanup. The fixture directory census is zero before
and zero after the test process. No environment values, live service or release
files were changed.

Independent read-only review was checked against the actual assertions and
callback. The proposed callback-before-permit-release sequence contradicted the
existing code and was rejected. Coverage does not establish in-flight HTTP
cancellation, database audit/quota effects, delivery of usage/archive callback
snapshots to persistence, FreeBuff/browser recovery or third-party background
task shutdown. These limits are retained in evidence/review.md.

Fresh all-targets, scoped rustfmt, checker 19/19, ratchet and separate Gateway/
Neuro diff checks pass. All compiler/formatter operations were serialized with
idle guards and GATEWAY_PREBUILT_WEB_UI=1. All owned native handles are terminal.
The log retains the existing production warning set and adds no test warning.
No dependency, checker policy, baseline or exception changed.

Strict scans 1,828 files: 17 hard, 24 mandatory, 40 soft; 41 remain above 700.
Clearance remains 104/145 (71.7%); strict exits 1. Global formatting was not
rerun for this test-only baseline; the preceding finalization acceptance records
the two reserved S06 failures, and every pinned production source is unchanged.

Immutable evidence: target/effective-line-evidence/20260914-pipeline-send-runtime/scope.json.
SHA-256: 684db7bb0d99badc272cba0966b8aac603682e6168431d9d500c02eb430a5620.
Gateway HEAD is 4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d: 2,044 entries
(182 modified, one unstaged deletion, two staged deletions, 1,859 untracked).
Neuro HEAD is bf818f0324024634bc890585efb78cc8e603d11a: 192 entries
(ten modified, 182 untracked). This census precedes documentation publication.

Next: prepare/review the stage-send control-flow projection, establish its
original inline/helper baseline and paired runtime baseline, then apply one
ownership boundary at a time. Preserve the existing callback body, permit and
admission order, fallback decisions, provider recovery and clone/move counts.
All completed source owners, including the entry, must be at most 500 effective
lines. S06 implementation/cursor and final freeze/build transfer remain reserved
under GWP-20260912-01. Full provider/language/release gates, immutable packaging,
packaged runtime/UI/Docker checks, persistent 4200 and no persistent 4226 remain
open. No release or persistent deployment occurred.

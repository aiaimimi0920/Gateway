# Provider quota transport diagnostics

Accepted: 2026-09-12 18:21:09 UTC. Gateway-only hardening checkpoint.

Accio and generic balance transport errors exposed query credentials through
rquest's attached request URL. Two real loopback connection failures reproduced
the leak before the production change. The unchanged eight-test contract advances
from six passes/two failures to 8/8 after the fix.

Codex, Accio and generic balance now call `without_url()` before formatting send
and body-read errors. These six statements are the entire production change.
Removing only those statements reconstructs each complete pre-fix probe file.
Request URLs, authentication headers, response parsing, message prefixes and
error category/status/code are unchanged. This strips the attached request URL;
it does not claim to redact arbitrary upstream bodies or underlying error sources.

| Scoped owner under src/provider_quota | Before | After |
| --- | ---: | ---: |
| entry (src/provider_quota.rs), test wiring only | 104 | 106 |
| codex.rs | 149 | 151 |
| accio_http.rs | 167 | 169 |
| generic_balance.rs | 116 | 118 |
| transport_error_contract.rs | new | 145 |
| transport_error_contract/fixture.rs | new | 90 |

Both new contract files and the entry's test-only wiring were frozen before the
red baseline and remain byte-identical. Eight production function signatures,
six structs and 249 neighboring inputs are preserved. The loopback suite covers
send/body-read failures, successful request wire behavior and quota projection,
HTTP 429 conflict handling and HTTP 503 classification. Codex's fixed external
endpoint receives source-level proof, existing unit coverage and compilation;
no live Codex request is claimed.

The fixtures use only synthetic credentials and random loopback ports, cap
header reads at 8,192 bytes, enforce server/join deadlines and abort their exact
owned task on drop. Requests are drained before assertions. No Redis, Docker,
real provider credential or persistent runtime was needed.

Fresh serialized gates pass: frozen contract 8/8; original quota units 7/7 in
both phases; `cargo check --offline --locked --all-targets`; scoped formatter;
checker 19/19; adoption ratchet; exact source/UTF-8/no-BOM proof; and both
repositories' `git diff --check`. Global formatting still reports only the
unchanged S06 runtime-mirror production/test files. All Cargo gates are terminal.

Strict scans 1,579 files: 31 hard, 30 mandatory and 40 soft; 61 remain above 700.
Clearance remains 84/145 (57.9%). No policy, baseline or exception changed.
Immutable evidence is `target/effective-line-evidence/20260912-provider-quota-diagnostics/scope.json`.
It binds both frozen snapshots, paired test cases/results, compiler/checker logs,
inventories, source/neighbor proof and separate repository observations.

At capture, Gateway has 1,739 dirty entries: 172 modified, one unstaged deletion,
two staged deletions and 1,564 untracked. Neuro has 192: ten modified and 182
untracked. Both HEADs and inherited deletions remain unchanged. Historical
accepted scopes stay immutable.

Credential stock/refill ownership is the next structural review. Quota response
body limits, provider-body diagnostic redaction, refresh-lock atomicity and
numeric conversion remain separate. S06 ownership, GWP-20260912-01 and the final
freeze/build transfer remain pending. No immutable release, packaged runtime/UI
acceptance or persistent deployment was performed.

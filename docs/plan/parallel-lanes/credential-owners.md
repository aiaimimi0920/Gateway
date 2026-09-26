# Credential cache ownership and routing contracts

Owner: parallel coordinator. State: accepted.
Started: 2026-09-12.

Scope is src/redis/credential_cache.rs, its new private credential_cache/
owners, src/routing/credential_routing.rs and its new private test owners.
The complete current sources and tests were read before selection. Entries
measure 976 and 866 effective lines. Existing module declarations and callers
remain outside the write scope.

Keep Redis CRUD, indexes, model lookup and expiry parsing in the cache entry.
Move credential types, runtime-material writeback and credential selection to
cohesive private owners. Preserve every production body, serialization shape,
public path, timestamp, TTL, Redis command/order, pipeline, isolation filter and
selection rule. CredentialKind::priority needs only family-local pub(super)
visibility after its type moves. Preserve all 14 cache tests and their fixture,
including the ignored test that requires live Redis at 127.0.0.1:6379.

Externalize routing tests into API/presets, search, media/browser sessions and
metadata/requirements groups. Preserve the production prefix, shared fixture and
all 26 tests, including the Qwen feature condition. Default-feature paired gates
must exercise the Qwen case. Do not use the ignored Redis test against a live
service or mix behavior changes into this extraction.

Evidence directory: target/effective-line-evidence/20260912-credential-owners/.
The immutable pre-edit snapshot contains 113 inputs. Projections measure
287/124/190/123/271 for the cache family and 216/36/151/158/167/144 for routing.
All 11 accepted owners match these projections and remain at most 287. Exact
comparison preserves 24 cache production items, 14 public paths, all 40 tests and
two fixtures, the Qwen feature condition and the normalized routing production
prefix. All 111 neighbors remain unchanged. Default-feature baseline/final tests
pass 13 with one ignored for cache and 26/26 for routing. Fresh all-targets,
scoped formatter, checker 19/19, ratchet, encoding and both Git checks pass.

The immutable scope was captured at 2026-09-12 02:14:44 UTC. Strict scans 1,500
files: 31 hard, 39 mandatory and 40 soft; 70 remain above 700. Clearance is
75/145 (51.7%). Global formatting still reports only the two unchanged S06
runtime-mirror files. All native gates are terminal.

[Acceptance report](../../status/2026-09-12-credential-owners.md).

S06 retains its source and original cursor. GWP-20260912-01 is still pending.
No release build, live-profile change or policy/baseline migration is included.

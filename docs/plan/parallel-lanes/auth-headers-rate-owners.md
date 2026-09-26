# Auth, request headers and rate-rule ownership

Date: 2026-09-21. State: structural_green.

The coordinator owns three independent soft-limit files in this batch:
`src/console/auth.rs` (617 effective), `src/http/request_headers.rs` (518),
and `src/rate_limit.rs` (516), plus their new nested owners. Keep the original
public module paths and test names. Preserve all implementation/test bodies.

- Auth: extract request/actor/view/record types and the existing tests. Keep
  authentication, grants, persistence and crypto runtime ownership in the parent.
  Only actor/record members actually accessed by the parent receive `pub(super)`;
  no external visibility grows.
- Request headers: move the existing test module intact. Keep the shared test
  auth fixture in the original parent path and preserve production code exactly,
  including the management-token gate for trusted forwarding/account selectors.
- Rate limiting: extract rule selection and canonical key construction. Retain
  the Redis Lua, timeout/admission mapping, types and store in the parent. Preserve
  key bytes, policy precedence, provider-attempt separation and public re-exports.

Evidence: `target/effective-line-evidence/20260921-auth-headers-rate-owners/`.
Run paired auth/header unit groups, rate-limit enforcement and scope-key external
contracts, all-target compilation, formatter, exact source/Lua proof, checker
tests/ratchet, strict audit, encoding and both Git checks. No dependency, policy,
baseline, exception or reserved S06 change. All resulting owners must be <=500.
The complete optimization and release goal remains active beyond this batch.

Completed evidence: `docs/status/2026-09-21-auth-headers-rate-owners.md`.
All seven resulting owners are <=359 effective lines. Paired tests pass
6/6, 13/13, 11/11 (two live Redis tests ignored), and 2/2; all-targets passes.
Strict soft entries decrease 38 -> 35; remaining >700 entries stay at 28.

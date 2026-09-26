# S3 listing continuation progress

Owner: parallel coordinator. State: accepted.
Started: 2026-09-12.
Accepted: 2026-09-12 05:41:08 UTC.

Scope is src/object_storage/listing.rs and a focused local HTTP regression file.
The earlier structural evidence stays immutable. The listing owner now measures
117 effective lines; its five-case HTTP regression owner measures 168.

The original S3 loop assigned next_continuation_token directly when is_truncated
was true. The real AWS SDK against an ephemeral loopback HTTP fixture reproduced
356 empty-token, 368 missing-token and 384 repeated-token requests before each
two-second deadline failed. The fixture retains at most four query maps and
aborts its owned server task on drop.

The five-test contract was captured before the production fix and remains
hash-identical. It preserves valid multi-page sorting, opaque token and prefix
forwarding, and termination on a non-truncated page carrying a token.

Missing, empty and immediately repeated continuation tokens now return the stable
object_storage_pagination_stalled error. Exact source proof preserves all other
methods, local traversal, valid request construction, sorting and signatures. This is
an immediate progress check; arbitrary longer cycles, total page/result budgets
and local blocking traversal remain separate boundaries.

The fixed contract advances from 2/5 to 5/5; all 14 original storage tests pass
in both phases. Final default-feature storage is 19/19. Fresh all-targets,
scoped formatter, checker 19/19, ratchet, source/172-neighbor proof, encoding and
both Git checks pass. All native gates are terminal. Strict scans 1,529 files:
31 hard, 35 mandatory and 40 soft; 66 remain above 700. Clearance stays 79/145.
The global formatter still reports only the two unchanged S06 mirror files.
No live S3, Redis, provider or release is involved.

Evidence: target/effective-line-evidence/20260912-storage-pagination/.
[Acceptance report](../../status/2026-09-12-storage-pagination.md).
S06 ownership and pending GWP-20260912-01 freeze/build coordination are unchanged.

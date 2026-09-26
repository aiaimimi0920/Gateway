# Folder-sync Redis Cluster hash-tag preparation, 2026-09-19

The folder-sync status and enabled override keys now use the shared Redis hash
tag `{provider-credential-folder-sync}`. The two-key `MGET` and Lua CAS paths
therefore address one Redis Cluster hash slot, which removes the known
cross-slot rejection for cooperating current Gateway writers.

## Compatibility

Readers fall back independently to the pre-change keys
`gw:provider-credential:folder-sync:status` and
`gw:provider-credential:folder-sync:enabled` when the new tagged key is absent.
New writers commit only the tagged keys. A legacy value is used as the value
being updated, but it is not deleted or atomically coordinated with arbitrary
legacy writers; a deployment migration must account for that boundary.

## Verification

- Redis key hash-tag unit test: 1 passed, 0 failed.
- Guarded status-store Redis suite after the key change: 8 passed, 0 failed.
- Guarded runtime Redis suite after the key change: 23 passed, 0 failed,
  including malformed-status startup fallback isolation.
- Complete folder-sync library after the key change: 140 passed, 0 failed,
  14 ignored.
- The Windows filesystem, watcher, and deletion gates remain 26/26, 41/41,
  and 9/9 respectively.
- `cargo check --offline --locked --all-targets` is rerun after this change.

## Disposable Redis Cluster protocol proof

A fresh three-node Redis 7 cluster was created on a random Docker network and
reported `cluster_state:ok` on all three nodes. The tagged status and enabled
keys both mapped to slot `15938`; the legacy status and enabled keys mapped to
slots `14601` and `4369`. A real two-key Lua `MGET` over the tagged keys
returned `status-value|enabled-value`, while the same `MGET` over the legacy
keys returned the expected `CROSSSLOT` error. All three containers and the
temporary network were removed after the probe.

This proves the key schema and Lua protocol at the Redis Cluster layer. The
Gateway still uses the regular `deadpool_redis::Pool` type, so a production
cluster-aware Gateway pool/client deployment is not claimed. Arbitrary
legacy-writer coordination, disconnect-window guarantees, remote S3
success/read, shared blocking-pool fairness, filesystem replacement races,
Unix/macOS behavior, and full provider/UI/Docker/release acceptance remain open.

# Folder synchronization status ownership

Status: verified status boundary, coordinator-owned S09 increment, 2026-09-15.

Parent 6655 -> 6441; new status owner 227. Paired folder_sync tests 45/45, all original
identities and exact full-source projections preserved. First candidate import
failure was corrected in candidate2 and retained as evidence. All-targets, scoped
formatter, checker 19/19, ratchet and both Git checks pass; native phases terminal.
Strict 2098/12/20/39; 32 above 700; clearance 113/145 (77.9%). The full parent remains
legacy debt. [Acceptance report](../../status/2026-09-15-folder-sync-status-owners.md).

## Design and verification boundary

The original plan assigns folder synchronization and keepalive to S09 separately
from S06 Gemini/media upstream leaves. Claim only provider_credential_folder_sync.rs
status types, configuration/default merging and Redis status/enabled persistence,
plus the new private provider_credential_folder_sync/status.rs owner. Preserve all
inherited root content and public import paths. S06 original source/cursor and final
release-build coordination remain reserved under GWP-20260912-01.

Before evidence follows the accepted ChatGPT solver scope d2352887eb35289039b5c7e08df3b82cbfb6ac65f9b3a7401adcf9e1e73dcb22.
The root starts at 6655 effective lines. The new status owner must be <=500; the
unmigrated root remains legacy debt and must decrease. No full root clearance is
claimed until its remaining responsibilities are actually migrated below 700.

Move the two public status/event DTOs, runtime-enabled resolution/public accessors,
config application, Redis status/enabled reads and writes, and watcher status
publication. Retain all function bodies, serde attributes, fields, public signatures,
Redis keys/commands, error text, ordering and parent calls. Only two internal
functions need pub(super) visibility. No wrapper compatibility layer is added;
public re-exports retain original paths. Watcher lifecycle and provider-specific
import/export/Gemini code stay outside this write batch.

Run the same broad folder_sync library filter before/after. Preserve its test
bodies and identities, with exact official-rustfmt source reconstruction. This is
structural work; do not add tests that merely duplicate unchanged implementation.
The existing tests do not automatically establish a new live Redis or packaged
runtime acceptance claim. Serialize native gates, freezing source/tests/assets.
Then run all-targets, scoped formatter, checker tests, ratchet, strict audit and
both repositories' staged/unstaged Git checks. Capture hashes and a read-only
semantic review. Full optimization/provider/runtime/UI/Docker/release remains open.

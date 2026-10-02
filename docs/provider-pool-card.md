# Provider pool lifecycle controls

The provider card back is split into three compact groups. Changes to thresholds,
automatic switches, local paths, and the storage password use the existing route
draft/commit flow. Requesting refill, deleting invalid credentials, or purging
archives requires a saved route configuration. The existing management session,
secret-access grant, and destructive-action confirmation rules remain in force.

## Capacity semantics

- `pool_target_size` is the backward-compatible maximum refill target; if omitted,
  it defaults to 100. `pool_min_size` defaults to 1. Existing explicit maximums
  are preserved. Values must be safe integers, with `0 <= min <= max` and `max >= 1`.
- Automatic refill starts only when enabled and the green available count is
  strictly below the minimum. A minimum of zero never starts automatic refill.
  An active refill cycle continues past the minimum toward the maximum.
- Unknown newly imported credentials and outstanding deliveries reserve capacity.
  Manual refill is subject to the same maximum, including when automatic refill
  is off. Requests and delivery admission recheck current configuration/capacity.
- The maximum is a **refill stopping target**, not an instruction to delete or
  disable existing accounts. Known cooling credentials may independently recover
  and put the available count above the maximum. New refill stops in that case;
  the recovered accounts remain intact.

## Storage and secrets

`credential_storage_path` is a provider-local directory of credential JSON files
supplied by refill workers. It is the input directory for the task's `folder_sync`
delivery mode, not an automatic export of the live account pool. Relative delivery
file names must remain inside that directory. The separate global account-folder
synchronization feature keeps its own existing behavior.

`credential_archive_path` is the provider-local recovery archive directory used
before deleting invalid credentials when permanent deletion is off. Omitted paths
retain the configured shared root/provider and root/_archive/provider defaults.
These are paths on the Gateway server, not on the browser's machine. Filesystem
access failures are errors; the UI does not claim a successful move or import.

HTTP/HTTPS URLs do not define storage operations, authentication, or conflict
semantics by themselves. This repository currently has no provider-pool cloud
storage protocol, so such addresses are rejected with a specific explanation.
Use a local or already-mounted filesystem directory until a protocol is agreed.

The existing `credential_storage_password` is a protected secret for external
storage/refill workers. It **does not encrypt local files**. Saved passwords are
not returned to the card; edits use the existing secret-patch authorization flow.
Local recovery archives remain plaintext JSON with existing restrictive file
permissions. The archive-password edit is unavailable until a supported storage
or encryption protocol exists; the UI does not save an inert password or imply
that files are encrypted.

## Deletion

Automatic deletion controls invalid-credential cleanup. Manual deletion asks the
trusted driver to identify invalid credentials. With permanent deletion disabled,
removed credentials first get recovery records. Enabling permanent deletion still
requires explicit confirmation. Purge deletes only the selected provider's archive
records, with a separate irreversible-deletion confirmation.

## Verification

All UI tests use mock APIs and fake credentials. The `Provider pool card UI`
workflow captures the actual React interface in desktop and mobile Chromium and
checks card flipping, focus, overflow, cancelled edits, and unsupported cloud-path
validation. It does not operate a live account pool or deploy a service.

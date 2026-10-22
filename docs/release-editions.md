# Gateway release editions

For daily functional validation, use the separate fast [devtest workflow](devtest-build.md).
The commands below describe explicitly requested formal release builds, not the daily default.

Gateway publishes one backend contract in three Windows editions:

| Edition | Cargo capability | Web management UI | `gateway-ui.exe` |
| --- | --- | --- | --- |
| `api` | `--no-default-features --features runtime-defaults` | No | No |
| `web` | Default features with `management-ui` | Yes, through `/ui/` | No |
| `desktop` | Default features with `management-ui` | Yes, through `/ui/` | Yes |

Build one edition with `tools/build-gateway-release.ps1 -Edition api|web|desktop`.
Build all three serially with `-Edition all`; the builder keeps edition-specific
backend snapshots so a later build cannot silently replace an earlier package.

Package with the same edition:

```powershell
.\tools\package-gateway-release.ps1 -Edition api -VersionId gateway-api-<id> -UseExampleRoutes
.\tools\package-gateway-release.ps1 -Edition web -VersionId gateway-web-<id> -UseExampleRoutes
.\tools\package-gateway-release.ps1 -Edition desktop -VersionId gateway-desktop-<id> -UseExampleRoutes
```

The package manifest records `edition` and both capability flags. API and Web
packages contain exactly one executable, even if a stale desktop executable is
left in the repository build tree. Release tree copying excludes test folders,
test/spec files, generated screenshots, caches, and temporary output; source
tests remain in the repository for development and CI.

## Runtime management UI switch

`gateway.example.json` contains the optional shared startup setting:

```json
{
  "enable_management_ui": true
}
```

The file is read from the selected `.ng` data root as `gateway.json`. An absolute
`GATEWAY_CONFIG_FILE` may point to another file. Missing files and missing fields
default to `true` for compatibility; malformed JSON, non-object JSON, or a file
larger than 64 KiB fails startup. The setting is read at process startup and
requires a restart after changes.

The effective value is the AND of the compile-time capability and this setting:

* `api` never exposes `/ui`, regardless of the file value.
* `web` exposes `/ui` only when the value is `true`.
* `desktop` creates its WebView only when the value is `true`; with `false` the
  launcher runs the local backend as an API-only process before Tauri startup.

The management API and its existing authentication boundary are independent of
this switch. A desktop connection configured for an existing remote server is
never changed into an implicit local server when the switch is disabled.

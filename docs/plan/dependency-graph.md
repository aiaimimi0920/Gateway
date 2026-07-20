# Gateway Productization Dependency Graph

```text
analysis documents
    |
    v
1A package layout + path resolver
    |
    +--> 1B profile/onboarding/lifecycle
    |        |
    |        v
    +--> 1C isolated packaged E2E
             |
             v
        2A inventory + evidence schema
             |
             +--> 2B offline matrix + opt-in canaries
             |        |
             |        v
             |   2C browser/credential operational evidence
             |
             v
        3A observability contract
             |
             +--> 3B dependency failure + access isolation
             |        |
             |        v
             +--> 3C recovery + operations + final release
```

## Dependency Rules

| Upstream | Downstream | Reason |
| --- | --- | --- |
| Package manifest and support-file policy | Desktop path resolution | The desktop resolver must know the staged layout before it can select a sidecar. |
| Profile role/token schema | Lifecycle drain and startup probes | Child environment and drain authorization are one contract. |
| Packaged runtime E2E | Provider inventory | Evidence must run against a known-good Gateway runtime. |
| Provider inventory schema | Canary runner and operator summary | Every canary result must map to one canonical line id. |
| Existing failure taxonomy | Evidence and SLO labels | External provider failures must not become Gateway implementation failures. |
| Request metrics registry | Fault-injection and operations manual | Recovery procedures need stable signals and thresholds. |
| Splitter lifecycle contract | Backup/restore and final release | Replacement and rollback must preserve state invariants before release. |

## Write-Scope Isolation

- Packaging lane owns `tools/package-*`, package tests, and release documentation.
- Desktop lane owns `apps/desktop/**` and desktop contract tests.
- Provider lane owns `tools/generate-*`, `tools/run-*`, `docs/provider-*`, and provider contract tests.
- Operations lane owns `src/metrics/**`, observability routes/tests, recovery tools, and operations docs.
- No lane edits the three user-modified media route files.


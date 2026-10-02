# Security release gates

## Scope

`.github/workflows/security.yml` is the shared dependency and secret-scan gate for:

- `build-windows.yml`: immutable Windows candidates;
- `docker.yml`: verified container images and optional GHCR publication;
- `release-tag.yml`: tagged Windows and Docker deployment bundles.

This closes the release dependency edge. It does not claim that every dependency
finding is fixed, replace runtime acceptance, or complete Neuro's broader
dependency-security rollout. CodeQL remains a separate source-analysis workflow.

## One source revision

The caller supplies the event commit, or the requested tag for a manually started
tag release. The policy job resolves that ref once and exports `source_sha`.
OSV and Gitleaks check out the resolved commit. Only a successful reusable
workflow permits the caller's build/publish job to run, and that job checks out
the same `source_sha`. Moving a branch or tag during scanning cannot silently
change the payload selected for publication.

Policy, dependency, or secret-scan failure/cancellation blocks the publishing job.
Do not add `always()` to bypass this dependency, suppress scanner errors, or
replace the checked SHA with a fresh branch/tag lookup. Existing tag-format
validation and package/runtime gates remain in force.

## Permissions and concurrency

Caller security jobs grant only `actions: read`, `contents: read`,
`pull-requests: read`, and `security-events: write`. The reusable workflow narrows
each job further. It does not inherit publishing credentials or write access to
repository contents/packages; checkouts do not persist credentials.

The security concurrency group includes the caller workflow name so standalone,
Windows, and Docker security runs do not cancel one another's pending jobs.

## Local checks and activation boundary

Run these contracts from the repository root:

```powershell
node --test scripts/tests/security-ci-policy.test.mjs scripts/tests/security-release-gates.test.mjs
```

The coverage contract enumerates all five committed first-party lockfiles and
preserves pinned security actions, redacted secret scanning, and fail-on-finding
behavior. The publication contract checks callers, permissions, resolved-SHA
handoff, failure dependencies, and separate concurrency groups.

The coverage contract also rejects withdrawn spin 0.9 patch versions through
0.9.8 in all three Cargo locks. This narrow regression guard does not replace
live registry metadata or full dependency scanning; the compatible 0.9.9 patch
and remaining findings are recorded in
[the dependency checkpoint](status/2026-09-30-dependency-spin-closure.md).

`crates/gateway-sqlx/Cargo.toml` 是由根 `Cargo.lock` 管理的内部 path dependency，
不是第六个独立构建/发布根。Dependabot 覆盖它的 manifest；
`tests/python/test_gateway_sqlx_dependency_contract.py` 保护其官方版本/features、
交付入口，以及根锁图不再引入 SQLx umbrella/MySQL/RSA 的约束。
桌面和 local-data 锁图保留独立 owner。验证范围和剩余问题见
[SQLx/RSA 迁移检查点](status/2026-09-30-sqlx-rsa-removal.md)；移除 RSA 不等于安全门禁通过。

Local contracts and YAML validation cannot prove a GitHub-hosted scan or release
has run. Activation requires reviewed commits on the relevant remote refs and
successful real GitHub runs with the repository's effective permissions. No
workflow should be described as remotely deployed from local checks alone.

The actual GitHub scan and browser-worker TLS dependency repair are recorded in
[the 2026-10-02 checkpoint](status/2026-10-02-browser-tls-security-closure.md).
The remaining desktop GTK advisories still block release until fixed or covered
by a genuinely approved, expiring advisory-level exception; no exception was
silently added by that repair.

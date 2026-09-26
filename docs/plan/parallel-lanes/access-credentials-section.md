# Access credentials section ownership

Date: 2026-09-24. State: structural_green.

Scope: the credentials accordion inside `AccessKeysWorkspace.tsx`. Keep
accordion state, credential drafts, issue/verify/revoke callbacks, and request
and secret ownership in the workspace facade. Move only the credential issue,
verify, revoke, and result JSX into a typed presentation owner. Preserve the
Section id, open/toggle behavior, field order, labels, translations, ARIA
roles, lock/busy disabled rules, and result formatting.

Result: the saved workspace was 384 effective lines and is now 183. The new
`AccessCredentialsSection.tsx` owner is 264 effective lines. Structural proof
confirms the moved section and facade-controlled accordion/callback wiring.
The full desktop suite passes 328/328 across 72 files; typecheck, Web build,
checker 19/19, ratchet, development-standard contract, and scoped diff checks
pass. Strict remains red only for the existing browser-profile payload debt.
Evidence:
`target/effective-line-evidence/20260924-access-credentials-section/` and
`docs/status/2026-09-24-access-credentials-section.md`.

This slice changes presentation ownership only. Credential secret handling,
API calls, persistence, Rust/S06, runtime, Docker, release, and live-provider
ownership remain outside this lane.

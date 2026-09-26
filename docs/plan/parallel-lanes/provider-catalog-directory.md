# Provider catalog directory ownership

Date: 2026-09-21. State: structural_green.

Scope: ProviderCatalogDialog.tsx (503 effective) and new
ProviderCatalogDirectory.tsx. Extract the controlled directory aside intact;
the parent keeps all state, filtering, form validation, secret input and submit
behavior. Pass current values/callbacks through typed props. Move the category
filter type with its view. Preserve DOM, classes, aria labels, button types,
translation calls and event bodies. No CSS, token or layout change.

Evidence: `target/effective-line-evidence/20260921-provider-catalog-directory/`.
Run paired dialog tests, typecheck, web build, exact source/JSX projection,
checker/ratchet/strict, independent review, encoding and separate Git checks.
Both source owners must be <=500 effective lines. Full-goal release gates and
the reserved S06 source/build scope remain open.

Result: 447/87 effective lines, paired 2/2 tests, typecheck, web build and exact
JSX/source projection pass. See `docs/status/2026-09-21-catalog-profile-owners.md`.

## Provider form ownership continuation (2026-09-24)

The controlled provider/account form JSX now belongs to
`ProviderCatalogForm.tsx` (221 effective lines). The dialog facade retains
template filtering and selection, draft initialization, all validation and
submit logic, and the Radix dialog lifecycle. The form receives typed draft
state/setters, selected-template data, secret-access state and the existing
translation/callback contracts. No API, secret, persistence or resource
ownership moved.

The saved 447-line dialog snapshot is now 276 effective lines plus the 221-line
form owner. Exact structural proof confirms the moved section and submit
handler, while the focused catalog suite remains 2/2. Full desktop is 328/328
across 72 files; typecheck, Web build, checker 19/19, ratchet and scoped diff
checks pass. Strict remains red only for the existing browser-profile payload
debt. Evidence: `target/effective-line-evidence/20260924-provider-catalog-form/`
and `docs/status/2026-09-24-provider-catalog-form.md`.

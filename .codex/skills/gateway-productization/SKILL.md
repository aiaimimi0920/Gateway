---
name: gateway-productization
description: Productize the Neuro Gateway across portable delivery, provider evidence, and enterprise runtime operations while preserving the canonical pipeline and development credentials.
---

# Gateway Productization

## Required Boundaries

- Work only under `C:\Users\Public\nas_home\AI\GameEditor\Neuro\Gateway`.
- Write releases only under `C:\Users\Public\nas_home\AI\GameEditor\Neuro\release\Gateway`.
- Never alter the existing uncommitted media route edits or unrelated subprojects.
- Do not remove, rotate, redact, variableize, or migrate development secrets requested by the owner.

## Delivery Order

1. Package and path contract.
2. Desktop profile, dependency preflight, and lifecycle.
3. Isolated packaged runtime E2E.
4. Provider inventory, evidence, and canaries.
5. Observability, reliability, access isolation, recovery, and final release.

## Test Discipline

For every production behavior change:

1. Add one focused failing test.
2. Run it and record the expected failure.
3. Implement the smallest change.
4. Run the focused test to green.
5. Run the affected suite and the full Gateway gate.

Do not make live upstream calls part of default CI. Use explicit opt-in flags and persist classified evidence.

## Preferred Commands

```powershell
python tools/validate-gateway-line-manifests.py
python -m unittest discover -s tests/python -p "test_*.py" -v
node --test scripts/tests/*.test.mjs
cargo test --manifest-path Cargo.toml --locked
Push-Location apps/desktop; npm run typecheck; Pop-Location
```

## Completion Standard

Do not report a phase complete based on source inspection alone. Require code, focused tests, affected tests, full validation, package contents, and isolated runtime evidence recorded in `docs/progress/`.


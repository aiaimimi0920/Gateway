# Credential document test ownership

Date: 2026-09-21. State: structural_green.

Scope: `apps/desktop/src/features/console/credentialDocument.test.ts` (636
effective lines), replacing it with creation, update/scheduling, deletion and
secret-patch test owners and one small shared route-document fixture. Preserve
all 13 test bodies, descriptions and assertions exactly. Retain Vitest's
existing automatic discovery and setup; change no production implementation,
test configuration, checker policy or baseline. Each owner must be <=500.

Evidence: `target/effective-line-evidence/20260921-credential-document-tests/`.
Run the original suite before moving, all four new suites afterward, desktop
typecheck, exact body/inventory projection, checker tests/ratchet/strict and
separate Gateway/Neuro Git checks. The desktop package has no formatter script;
retain source formatting rather than introducing a formatting dependency.
This scope is independent of the running Rust candidate compile and reserved
S06 sources/build window. The full optimization/release goal stays active.

Result: four automatically discovered suites retain all 13 tests, paired tests
and desktop typecheck pass; all five owners are <=199 effective lines. The
original filename retains creation tests. See
`docs/status/2026-09-21-routing-credential-test-owners.md` for full evidence.

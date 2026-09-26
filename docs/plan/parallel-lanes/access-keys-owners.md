# Access keys workspace ownership

## 2026-09-10 checkpoint

`AccessKeysWorkspace.tsx` now retains workspace state, draft patch callbacks,
section composition and the public export surface. Its ratchet entry decreased
from 1253 to 405 effective lines, below the 500-line preferred boundary.

The extracted owners are:

- `accessKeysTypes.ts`: draft types, defaults, public workspace props, scope
  parsing and API input builders.
- `AccessWorkspacePrimitives.tsx`: copy feedback, metric cards, accordion
  sections and one-shot secret presentation.
- `AccessWorkspaceHeader.tsx`: refresh toolbar and secret-banner placement.
- `AccessKeysSection.tsx`: access-key form, bundle selection and key ledger.
- `AccessBundlesSection.tsx`: bundle creation form and bundle ledger.
- `AccessAffinitySection.tsx`: sticky-affinity inspection/reset and project-key
  rotation controls.
- `useAccessKeysViewModel.ts`: catalog sorting, bundle bindings, counts and
  form completeness derivation.
- `accessWorkspaceFormatting.ts`: display-safe counts, timestamps, statuses and
  identifier masking.

`useAccessData.ts` imports the contract helpers from `accessKeysTypes.ts`, while
`AccessKeysWorkspace.tsx` re-exports them for existing callers. No route or API
behavior was changed.

Measured effective-line owners: parent `405`; key section `289`; affinity
section `213`; bundle section `164`; primitives `168`; contract types `198`;
header `45`; view-model `62`; formatting `40`; and the focused test `75`.

The focused `accessKeysTypes.test.ts` suite passes 3/3 and protects whitespace
scope parsing, nullable bundle/key fields and the positive credential-duration
fallback. The consolidated desktop suite passes 314/314 across 62 files; web
build, TypeScript typecheck, effective-line checker tests (19/19), ratchet and
`git diff --check` also pass. All new owners are below 500 effective lines, so no
soft-limit exception was needed for this lane.

Native Rust/Cargo packaging is outside this UI lane and remains gated by the
explicit GWP-20260908-06 source/docs freeze and Cargo transfer receipt.

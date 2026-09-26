# Provider catalog form ownership continuation

Date: 2026-09-24 UTC. Status: structurally verified.

The coordinator continued the existing provider-catalog directory split by
moving the controlled provider/account form presentation into
`ProviderCatalogForm.tsx`. `ProviderCatalogDialog.tsx` retains the dialog
lifecycle, template filtering and selection state, draft initialization,
validation, submit ordering, and public props. The new form owner receives
typed draft/setter values and renders the existing form JSX, including the
secret-access warning and Radix cancel/submit controls.

The saved pre-edit dialog was 447 effective / 467 physical lines. The current
owners are:

- `ProviderCatalogDialog.tsx`: 276 effective / 292 physical lines;
- `ProviderCatalogForm.tsx`: 221 effective / 228 physical lines.

`target/effective-line-evidence/20260924-provider-catalog-form/verify-structure.mjs`
proves that the moved provider-form section is exact and that the dialog's
submit handler remains exact, including validation order and callback ordering.
The extraction introduces no new request, secret read, persistence path, or
resource lifecycle.

Verification for this continuation:

- `ProviderCatalogDialog.test.tsx`: 2/2 passed;
- full desktop Vitest: 72 files, 328/328 tests passed;
- desktop TypeScript typecheck and Web build passed;
- effective-line checker tests passed 19/19 and the ratchet passed;
- strict audit remains intentionally red only for the existing browser-profile
  payload debt; no first-party violation was introduced;
- encoding/no-BOM and scoped diff checks pass.

No formatter is configured for the desktop package. No Rust/Cargo, Docker,
provider, runtime, or immutable release gate was run for this UI presentation
extraction.

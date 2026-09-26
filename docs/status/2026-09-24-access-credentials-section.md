# Access credentials section owner

Date: 2026-09-24 UTC. Status: structurally verified.

The coordinator moved the credential issue, verify, revoke, and result
presentation out of `AccessKeysWorkspace.tsx` into the typed
`AccessCredentialsSection.tsx` owner. The workspace facade keeps accordion
state, the credential state and callbacks, and all request/secret ownership.
The child receives those existing values and callbacks only; it does not add a
request, persistence, timer, secret read, or resource lifecycle.

The saved pre-edit workspace was 384 effective / 395 physical lines. The
current owners are:

- `AccessKeysWorkspace.tsx`: 183 effective / 192 physical lines;
- `AccessCredentialsSection.tsx`: 264 effective / 270 physical lines.

`target/effective-line-evidence/20260924-access-credentials-section/verify-structure.mjs`
compares the moved `<Section id="credentials">` presentation against the
saved snapshot, normalizing only owner indentation and the facade-controlled
`onToggle`/`open` boundary. It proves that the accordion remains controlled by
the facade, all credential callbacks remain wired once, and the parent no
longer contains the moved markup or patch helpers.

Verification for this continuation:

- full desktop Vitest: 72 files, 328/328 tests passed;
- desktop TypeScript typecheck and Web build passed;
- structural proof passed;
- effective-line checker tests passed 19/19 and the ratchet passed;
- strict audit remains intentionally red for the existing 12 browser-profile
  payload violations (2,480 files scanned; 0 first-party violations);
- Neuro development-standard contract passed;
- Neuro root and Gateway scoped `git diff --check` passed;
- the source owners and evidence script are UTF-8 without BOM and have no
  trailing whitespace.

No desktop formatter is configured. No Rust/Cargo, Docker, provider, runtime,
or immutable release gate was run for this presentation-only extraction.

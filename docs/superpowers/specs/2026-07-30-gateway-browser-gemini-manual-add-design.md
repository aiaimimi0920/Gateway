# Gateway Browser Gemini Manual Add Design

## Goal

Add a browser-first Gemini credential bootstrap flow to the Gateway web console so operators can click a Gemini credential family row, launch a local manual-auth helper, and merge the generated credential material back into the current route-config draft without leaving the browser UI.

## Scope

This design covers the first browser implementation for Gemini manual add inside the existing account ledger UI.

- In scope
  - Browser console buttons on Gemini credential-family rows
  - Local Gateway backend auth-session endpoints
  - Canvas auth flow that opens a visible local browser and exports runtime state
  - Business auth flow that opens a visible local browser and captures runtime material from an intercepted widget request
  - Success path that materializes draft credentials for both `gemini-canvas` and `gemini-canvas-chat`
  - Success path that materializes a `gemini-business` credential plus secret patch material for its JWT
  - Progress UI and polling inside the browser console
- Out of scope for this slice
  - Non-browser management surfaces
  - Commit/publish workflow changes

## Constraints

- The browser UI is the primary operator experience.
- Pure browser JavaScript cannot directly read Google/Gemini cookies or local browser state, so the browser UI must call local Gateway backend endpoints which spawn repo-side helpers on the same machine.
- The current web console edits a route-config draft in memory; the new flow must merge generated credentials into that draft instead of bypassing the editor.
- The workspace is already dirty, so changes must remain additive and targeted.

## Existing Building Blocks

- `apps/desktop/src/features/console/BrowserConsoleApp.tsx`
  - Owns the route-config draft, dialogs, and account-ledger actions.
- `apps/desktop/src/features/console/AccountsLedgerWorkspace.tsx`
  - Renders provider/category/account rows and per-row actions.
- `apps/desktop/src/features/console/credentialDocument.ts`
  - Already supports adding and updating explicit provider credentials in the draft.
- `src/http/routes/internal_console.rs`
  - Existing authenticated web-console backend surface.
- `scripts/export-gemini-canvas-storage-state.mjs`
  - Already launches a visible Chromium/Edge session, waits for manual Gemini login, exports storage state, and prints JSON with `runtimeStateObjectKey` and `suggestedShareId`.

## Functional Design

### 1. Session-based backend control plane

Add console-scoped Gemini auth sessions with:

- `POST /v1/internal/gateway/console/gemini-auth-sessions`
- `GET /v1/internal/gateway/console/gemini-auth-sessions/:session_id`

Each session records:

- target family
- related provider id
- lifecycle status: `pending | waiting_user | succeeded | failed`
- operator-facing status message
- timestamps
- generated draft credential payloads on success

The backend authenticates the management token before creating or reading sessions.

### 2. Canvas-first generation behavior

When the user clicks manual add on either `gemini-canvas` or `gemini-canvas-chat`, the backend runs the existing `export-gemini-canvas-storage-state.mjs` helper and, on success, produces two generated draft credentials:

- one explicit credential for `gemini-canvas`
- one explicit credential for `gemini-canvas-chat`

Both credentials share the exported `runtime_state_object_key`.

The chat credential also includes the existing `apiBaseUrl` extra-body default used by the preset examples.

### 3. Browser console UX

Gemini family rows get a `手动添加 / Manual add` action.

Clicking it opens a modal that:

- starts a Gemini auth session
- shows current status and instructions
- polls the session until it succeeds or fails
- applies returned generated credentials to the current draft
- surfaces a success message telling the operator the draft was updated and still needs saving

### 4. Gemini Business behavior in this slice

Gemini Business uses the same browser-first entry point and session plumbing, but its helper differs from Canvas:

- the helper opens `business.gemini.google`
- the operator signs in and triggers one real Gemini Business action
- the helper intercepts the outbound widget request and extracts:
  - bearer JWT
  - `configId`
  - `session`

The browser console writes the public runtime fields into the draft credential and keeps the JWT in the existing secret-edit pipeline so the final save still flows through console secret patches.

## Data Flow

1. User opens account ledger.
2. User clicks manual add on a Gemini family row.
3. Browser console calls `createGeminiAuthSession`.
4. Backend creates a session and spawns the local helper worker.
5. Browser console polls `getGeminiAuthSession`.
6. Worker succeeds and session stores generated draft credential payloads.
7. Browser console merges generated credentials into the current draft and updates transient secret-edit state if needed.
8. User reviews and saves the route-config draft through the normal console flow.

## Error Handling

- Invalid or missing management token: reuse existing console auth errors.
- Script spawn failure: session becomes `failed` with the spawn error message.
- Manual login timeout: session becomes `failed` with the helper timeout message.
- Draft parse failure at merge time: keep the session result, show the merge error, and do not mutate the draft.
- Duplicate credential ids: treat as update-in-place for generated Gemini credentials when the same id already exists under the target provider.

## Testing Strategy

### Frontend

- Console API client tests for new create/get endpoints.
- Browser console tests that:
  - render manual-add actions on Gemini rows
  - start a session when clicked
  - poll status
  - merge generated Canvas credentials into the draft
  - surface structured backend failure for Gemini Business

### Backend

- Unit tests for the Gemini auth-session manager:
  - session creation
  - Canvas output-to-generated-credential translation
  - business unsupported-session result

## Deliverable for this slice

A browser-first Gemini manual-add flow that is usable today for all three Gemini credential families:

- one Canvas manual login adds both Canvas credential families
- one Gemini Business manual login + one real business request captures the JWT/config/session material
- all generated credentials land in the current browser draft and follow the normal save path

# Gateway Browser Gemini Manual Add Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a browser-first Gemini manual-add flow to the Gateway web console, with Canvas auth generating both Canvas credential families and a structured backend response for Gemini Business.

**Architecture:** Extend the existing internal console API with authenticated Gemini auth sessions backed by a small in-memory session manager. The browser console starts a session from the account-ledger UI, polls for progress, and merges generated credentials into the current route-config draft with existing credential-document helpers.

**Tech Stack:** Rust, axum, tokio process spawning, TypeScript, React, Vitest, MSW

---

### Task 1: Define the API contract and session-manager boundaries

**Files:**
- Modify: `Gateway/apps/desktop/src/api/contracts.ts`
- Modify: `Gateway/apps/desktop/src/api/schemas.ts`
- Modify: `Gateway/apps/desktop/src/api/console.ts`
- Create: `Gateway/src/console/gemini_auth_sessions.rs`
- Modify: `Gateway/src/console/mod.rs`

- [ ] **Step 1: Add failing frontend API contract tests**

Add tests to `Gateway/apps/desktop/src/api/console.test.ts` for:

- `POST /v1/internal/gateway/console/gemini-auth-sessions`
- `GET /v1/internal/gateway/console/gemini-auth-sessions/:id`

Expected assertions:

- management token header is forwarded
- request body includes `targetFamily` and `providerId`
- parsed response exposes `status`, `message`, and generated credential payloads

- [ ] **Step 2: Run the focused API tests and verify they fail for missing methods**

Run:

```powershell
pnpm --dir Gateway/apps/desktop test -- --run src/api/console.test.ts
```

Expected: failure pointing to missing Gemini auth-session API surface.

- [ ] **Step 3: Add TypeScript contracts, schemas, and client methods**

Implement:

- Gemini auth family union
- create/get request/response types
- zod schemas
- `createGeminiAuthSession(...)`
- `getGeminiAuthSession(...)`

- [ ] **Step 4: Add the Rust session-manager skeleton**

Create `Gateway/src/console/gemini_auth_sessions.rs` with:

- request family enum
- session status enum
- session view structs
- an in-memory manager API for `create`, `get`, and background completion hooks

- [ ] **Step 5: Re-run the focused API tests**

Run:

```powershell
pnpm --dir Gateway/apps/desktop test -- --run src/api/console.test.ts
```

Expected: API tests pass.

### Task 2: Implement the backend Canvas session flow

**Files:**
- Create: `Gateway/src/console/gemini_auth_sessions.rs`
- Modify: `Gateway/src/http/routes/internal_console.rs`
- Modify: `Gateway/src/http/router.rs`

- [ ] **Step 1: Add failing Rust unit tests for session creation and Canvas translation**

Cover:

- creating a Canvas session returns `waiting_user`
- translating helper output returns generated credentials for both `gemini-canvas` and `gemini-canvas-chat`
- business sessions fail with a structured unsupported message

- [ ] **Step 2: Run the focused Rust tests and verify they fail**

Run:

```powershell
cargo test --manifest-path Gateway/Cargo.toml gemini_auth_sessions -- --nocapture
```

Expected: failure because the session manager does not yet implement the behavior.

- [ ] **Step 3: Implement the worker launch and session updates**

Implement:

- Canvas helper spawn using `node` + `scripts/export-gemini-canvas-storage-state.mjs`
- session status transitions
- helper stdout JSON parsing
- generated draft credential material for both Canvas families

- [ ] **Step 4: Expose authenticated console routes**

Add handlers for:

- create Gemini auth session
- read Gemini auth session

- [ ] **Step 5: Re-run the focused Rust tests**

Run:

```powershell
cargo test --manifest-path Gateway/Cargo.toml gemini_auth_sessions -- --nocapture
```

Expected: focused session-manager tests pass.

### Task 3: Wire the browser console UI

**Files:**
- Modify: `Gateway/apps/desktop/src/features/console/AccountsLedgerWorkspace.tsx`
- Modify: `Gateway/apps/desktop/src/features/console/BrowserConsoleApp.tsx`
- Modify: `Gateway/apps/desktop/src/features/console/credentialDocument.ts` (only if a merge helper reduces duplication)

- [ ] **Step 1: Add failing browser-console tests for Gemini manual add**

Add tests covering:

- Gemini rows show `手动添加 / Manual add`
- clicking Canvas manual add starts the session and eventually writes both Canvas credentials into the JSON draft
- clicking Business manual add surfaces a structured unsupported message

- [ ] **Step 2: Run the focused browser-console tests and verify they fail**

Run:

```powershell
pnpm --dir Gateway/apps/desktop test -- --run src/features/console/BrowserConsoleApp.test.tsx
```

Expected: failure because the UI does not yet render the new action or session dialog.

- [ ] **Step 3: Add the Gemini manual-add action to the ledger rows**

Render the action only for supported Gemini families:

- `gemini-canvas`
- `gemini-canvas-chat`
- `gemini-business`

- [ ] **Step 4: Implement the session dialog, polling, and draft merge**

In `BrowserConsoleApp.tsx`:

- create manual-add dialog state
- start session on click
- poll until terminal status
- merge generated credentials into the draft
- update success/error status text

- [ ] **Step 5: Re-run the focused browser-console tests**

Run:

```powershell
pnpm --dir Gateway/apps/desktop test -- --run src/features/console/BrowserConsoleApp.test.tsx
```

Expected: focused UI tests pass.

### Task 4: Verify the end-to-end slice

**Files:**
- Modify: `Gateway/docs/superpowers/specs/2026-07-30-gateway-browser-gemini-manual-add-design.md` (only if implementation reveals a required correction)
- Modify: `Gateway/docs/superpowers/plans/2026-07-30-gateway-browser-gemini-manual-add.md` (only if implementation reveals a required correction)

- [ ] **Step 1: Run the targeted frontend and backend verification set**

Run:

```powershell
pnpm --dir Gateway/apps/desktop test -- --run src/api/console.test.ts src/features/console/BrowserConsoleApp.test.tsx
cargo test --manifest-path Gateway/Cargo.toml gemini_auth_sessions -- --nocapture
```

Expected: all targeted tests pass.

- [ ] **Step 2: Do a minimal compile-oriented backend check**

Run:

```powershell
cargo test --manifest-path Gateway/Cargo.toml internal_console -- --nocapture
```

Expected: route module and related console code compile cleanly.

- [ ] **Step 3: Self-review the slice against the spec**

Check:

- Canvas adds both credential families from one auth
- browser UI is the primary operator surface
- business is not exposed as a dead button
- no unrelated account-ledger behavior regressed


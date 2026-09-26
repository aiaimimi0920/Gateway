# ChatAIBot browser probe ownership

## Scope and structure

The coordinator owns this isolated extraction of scripts/chataibot-session-worker.mjs
and scripts/chataibot-session/probe.mjs. No existing S06, ChatGPT, Qwen, Suno,
Udio, AI Studio or package-contract lane source was modified.

The browser-side session probe now has its own module. It remains self-contained
for Playwright serialization; browser startup, profile cloning, credential-file
writing, stdout schema and process lifetime remain in the worker entrypoint.
Existing JavaScript worker conventions are retained without adding a build step.

Repository-lexer effective lines:

- Worker entrypoint: 628 -> 405.
- Browser probe: 225.
- Focused regression tests: 103.

All three files are UTF-8 without BOM. The extracted body is byte-exact after
removing its original four-space nesting indentation. Reconstructing the original
entrypoint confirms that its only changes are the import and evaluate call.
No formatter script is supplied for these worker modules; original body formatting
was retained and syntax/whitespace checks passed.

## Verification

Five offline tests passed against the original inline callback before extraction
and against the exported callback afterward. The candidate executes the function's
serialized text in a fresh VM browser context, preventing Node closure dependencies
from being hidden by imports. Tests cover:

- Missing authentication, no quota request, and IndexedDB close.
- Cookie precedence, JWT account/expiry mapping, model selection and zero quota.
- Preferred localStorage keys before arbitrary keys and IndexedDB.
- Nested IndexedDB token and non-JSON failed quota response.
- Forty-row-per-store scan boundary.

Fresh commands, all exit zero, from Gateway with rtk:

- node --test scripts/tests/chataibot-session-probe.test.mjs: 5 passed.
- node --check scripts/chataibot-session-worker.mjs.
- node --check scripts/chataibot-session/probe.mjs.
- npm run test:effective-lines --prefix scripts: 19 passed.
- npm run check:effective-lines --prefix scripts: ratchet passed.
- python -m unittest discover -s tests/python
  -p test_gateway_nested_worker_package_contract.py -v: 1 passed.
- git diff --check, separately in Gateway and Neuro: passed.

A read-only independent reviewer found no new regression and confirmed browser
closure independence, IndexedDB close ownership and the bounded cursor iteration.
This is not real-provider or Chromium runtime validation. The existing packaging
contract passes but does not explicitly enumerate the new ChatAIBot module;
the production packager recursively copies the complete scripts tree.

The fresh ratchet scans 2249 files: 10 above 1500, 18 between 701 and 1500,
26 between 501 and 700. This batch clears one soft-limit debt file. The remaining
28 files above 700 are unchanged by this scope. Policy, baseline and exceptions
were not changed.

## Safety and lifecycle review

No network, storage or recursive scan was added; token precedence, returned secret
fields, quota request and database cleanup retain original semantics. Tests use
synthetic tokens and do not read real profiles, credentials or provider sessions.

Confirmed existing cleanup defect: printAndExit calls process.exit before the
outer finally can close the browser or remove the profile clone. This needs a
separate failure/cleanup regression batch. Other retained limitations include
unbounded stdin/profile copying, no fetch deadline, IndexedDB transaction abort
handling, and broad JWT candidate selection. These are not accepted as hardened
merely because the structural extraction passed.

## Continuation

The next useful batch is worker exit/cleanup ownership with subprocess proof for
success and failure paths. Full strict closure, integrated native verification
and final release remain open. S06 and final native build ownership remain reserved
pending a real transfer receipt. No release/Gateway publication was performed.

Gateway status remained dirty (210 tracked entries and 690 untracked entries at
the pre-report checkpoint); Neuro separately remained dirty (10 tracked and 34
untracked entries). Existing changes/staging were preserved. No commit, push,
sibling source change or live-service mutation was performed.

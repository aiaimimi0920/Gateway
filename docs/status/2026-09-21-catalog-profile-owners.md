# Catalog directory and desktop profile ownership

Date: 2026-09-21. Status: structural_green; complete optimization/release goal active.

## Provider catalog directory

ProviderCatalogDialog.tsx decreases from 503 to 447 effective lines. The new
ProviderCatalogDirectory.tsx (87) renders the controlled search/category/template
aside. All state, filtering, selection, form validation, secret input and submit
logic stay in the dialog. The directory receives no draft or API key.

The extracted JSX is identical apart from indentation: no wrapper, DOM class,
ARIA attribute, button type, translation string or event body changes. The
existing Neuro UI tokens, scroll boundaries and Radix dialog lifecycle remain
unchanged. The parent import was moved to its normal import group after review.

Evidence: `target/effective-line-evidence/20260921-provider-catalog-directory/`.
Original and extracted dialog suites each pass 2/2, exercising custom-provider
creation and existing-provider account selection. Typecheck and web build pass
after the final import edit. Exact parent/child projection and UTF-8/no-BOM pass;
independent review found no regression. No screenshot or full visual/UI release
acceptance is claimed from these jsdom tests and unchanged markup.

## Desktop profile state

| File under `apps/desktop/src/state/` | Effective lines | Responsibility |
| --- | ---: | --- |
| `useGatewayDesktopState.ts` | 421 (was 627) | Process/probes/logs/API tests, diagnostics, busy/notice state, polling and auto-start |
| `useGatewayProfileState.ts` | 265 | Profile selection/ref, draft/saved snapshot, validation, transfer, persistence, path checks and templates |
| `desktopNotice.ts` | 10 | Existing notice factory and error-to-message conversion |
| `useGatewayDesktopState.test.tsx` | 152 | Five observable state-transition contracts |

Parent React setters remain stable inputs to the profile hook. A stable parent
callback invalidates API/health/ready/model results after the child invalidates
its path result. The selected-profile ref sync effect runs before the parent
auto-start reset effect; immediate ref assignment still precedes selected-name
state assignment. StartGateway keeps save -> snapshot update -> sidecar start
-> selection/reload -> runtime refresh order in the runtime owner.

Exact source projections pass for all three owners, including the explicit
invalidation delegation and split ref/reset effect. All other moved action
bodies, validation, notices and callback dependencies are retained. There is no
reverse module import or new timer, queue, process or persistence path.

Three contracts were added and passed against the original implementation:
successful save/invalidation, failed save retaining a dirty draft and clearing
busy, and default-profile reset without deletion. The original two auto-start
contracts remain unchanged. All five pass after extraction; desktop typecheck
and web build also pass.

Evidence: `target/effective-line-evidence/20260921-desktop-profile-owner/`.
An initial reviewer missed the ignored baseline and compared HEAD; its baseline
comparison was not accepted as authoritative. A separate reviewer directly read
the saved baseline and verified current ref/callback, invalidation, reload,
save/delete and start ordering with no extraction regression. The main thread
also verified the saved baseline bytes against the captured source.

## Common validation and remaining scope

Both evidence directories retain baseline sources and extraction-proof.json
hashes/counts. Checker tests pass 19/19, ratchet passes, all touched source/test
files are UTF-8 without BOM, and separate Gateway/Neuro diff checks pass. The
desktop package has no configured formatter; existing source formatting is
preserved. No Rust source changed or Rust compile gate was claimed this batch.

Strict inventory changes 2238 -> 2241 scanned and 31 -> 29 soft entries.
Hard/mandatory counts remain 10/18, with 28 files above 700. The first strict
invocation emitted a PowerShell host NullReferenceException; rerunning the same
checker directly with Node produced fresh matching JSON and the expected debt
exit 1. The host failure and retry are retained separately.

Inherited async profile-selection/refresh races, request cancellation and error
coverage require separate investigation; the extraction does not fix or expand
them. Tests do not prove concurrent profile switching, native Tauri sidecar
acceptance, clipboard behavior, every template or a packaged release.

Gateway's inherited dirty worktree and Neuro's Gateway submodule state are
preserved. No sibling source, checker policy/baseline, exception, profile,
staging, commit, push or live service was changed. The web build is local only;
no new package was placed in `C:\Users\Public\nas_home\AI\GameEditor\Neuro\release\Gateway`.

Next desktop candidates: ModelPoolWorkspace.tsx (624) and
apps/desktop/tools/publish-web-dist.mjs (613). The model pool scout identified
workspace state/focus/menu ownership versus card rendering, but a single large
card extraction would need measurement and possibly a second cohesive boundary.
Full-goal gates, S06 source/build transfer and runtime-artifact governance remain
open in `2026-09-21-refactor-completion-audit.md`.

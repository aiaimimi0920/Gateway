# Desktop stylesheet ownership acceptance, 2026-09-15

The Gateway-only structural scope is accepted. Full optimization and release work
remain open. styles.css decreased from 5,545 to 30 effective lines; 30 cohesive
owners contain the original rules, with a maximum of 441 effective lines.

The original 6,616 physical lines were read before selecting complete rule
boundaries. Every selector, declaration, media query, keyframe, theme token and
unlayered cascade position reconstructs exactly. main.tsx keeps its original
styles.css import. Only owner-purpose comments and inter-file boundary whitespace
were added. Seven owners occupy the permitted 251-500 range with one UI-domain
responsibility each. Tiny late override owners retain precedence at their original
positions. There are no new layers, dependencies, features or visual changes.

The existing TypeScript theme suite and four Python desktop contract suites now
expand actual flat imports. All assertions are preserved. Readers reject nested
or unsupported imports; the shared Python native-module reader is unchanged.
The first candidate TS reader lacked regex multiline mode and failed during suite
load, executing zero tests. Its g-to-gm correction passed retry1 without changing
production CSS. Both failed and passing receipts are retained.

## Verification

- Theme Vitest: 8/8 before and 8/8 after, identical test identities.
- Python desktop source contracts: 52/52 before and after, identical identities.
- TypeScript check: passes before and after.
- Web and Tauri frontend builds: pass before and after. Each target's complete
  11-artifact path/hash census is identical. These Rsbuild targets do not establish
  a native Tauri executable or bundle.
- Both targets emit static/css/index.f3c7b775.css with SHA-256
  002833d2ef958b7082ba7199f954f34caee6e51ad97b566b781ef48b0d030094.
- Real headless Edge serves the actual built web frontend from an ephemeral
  loopback server. Existing API mocks cover administrator bootstrap, accounts
  and settings at widths 390/900/1440 in dark/reduced-motion mode. All six
  observations and six baseline/candidate PNG pairs are exactly equal; page
  errors are zero. Main opened narrow accounts and wide settings on both sides.
- All 30 owners parse independently with installed PostCSS (914 top-level nodes,
  including comments). Source projection, UTF-8/no-BOM and scoped line checks pass.
- Checker tests 19/19, ratchet and Gateway/Neuro staged and unstaged diff checks
  pass. Strict exits 1 for remaining debt. Gates were serialized with frozen inputs.
- Two independent read-only reviewers found no introduced defect. Coverage limits
  were accepted explicitly. No official formatter is configured for this scope;
  existing parser/typecheck/encoding/whitespace/diff checks were used.

Browser proof covers mocked product UI. The accounts metric HTTP 404 warning and
empty card region appear identically in baseline and candidate. No live backend,
all-dialog/state/theme, accessibility, native desktop, Docker or release success
is inferred. Browser contexts, browser and ephemeral server close in finally;
both browser commands exited successfully. No persistent service was created.

## Evidence and remaining work

Evidence root: target/effective-line-evidence/20260915-desktop-style-owners/.
Acceptance: scope.json at 2026-09-15T07:03:33.826Z.
Scope SHA-256: d2c23bd4db0f3bc837bc29a4d4399cc74a6e983e426506933b085635e5ea93fe.
Predecessor: 6505ddf1aaa14897ddf239e48d3d0c436b7eb7851e1b6f4191fde3b0f7d80b54.
Accepted union: 1200 input files; 37 scoped files; 1163 unchanged neighbors.
scope.json records exact source hashes, test identities, build manifests, browser
comparisons, parser results, review hash, gate receipts and repository observations.
publication.json records the final source/document hashes and both Git states.

Fresh strict inventory: 2073 scanned, 12 hard, 22 mandatory, 40 soft. The count
above 700 falls from 35 to 34; clearance is 111/145 (76.6%). Remaining findings
above 700 are 22 Rust files and 12 runtime-profile/vendor files. Existing
runtime-profile/vendor data remains untouched. Prior image-edit 511, browser-pool
932 and exporter 25 test results are retained with frozen inputs, not rerun here.

At acceptance, Gateway HEAD is 4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d:
186 modified, 1 unstaged deletion, 2 staged deletions and 2260 untracked paths.
Neuro HEAD is bf818f0324024634bc890585efb78cc8e603d11a:
10 modified and 182 untracked paths. Existing dirt and staging are preserved;
final publication may include the new status document as another untracked path.

Before closing, three Cargo/rustc processes were observed. Their command lines
had no Gateway indicator; cwd was not established. No native/frontend build was
started or stopped during closing; external processes were preserved. S06's
original Rust/Gemini scope and final native-build/release coordination remain
reserved under GWP-20260912-01. Continue strict/language/provider/packaged runtime/
full UI/Docker/release gates. Persistent target 4200; no persistent 4226. No Rust,
credentials, runtime profiles, dependencies, checker/baseline/policy/exceptions,
sibling source, release payload, deployment or commit changes in this scope.

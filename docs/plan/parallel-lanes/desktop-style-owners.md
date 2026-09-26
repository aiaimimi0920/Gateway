# Desktop stylesheet ownership

Coordinator-owned structural scope, accepted 2026-09-15 after the image-edit broad
lifetime checkpoint. The entry decreased from 5,545 to 30 effective lines through
30 domain stylesheets, maximum 441. Complete original rules and the unlayered
cascade reconstruct exactly through the existing styles.css entry.

Scope includes styles.css, new styles/*.css owners, and the TypeScript/Python
source-contract readers. Every existing assertion, selector, declaration, media
query, keyframe, theme token and main.tsx import is preserved. The readers expand
actual flat imports and reject nested/unsupported imports. No design changes.

Paired theme 8/8, Python contracts 52/52, typecheck and web/Tauri frontend builds
pass. Both complete 11-artifact build censuses match exactly before and after.
Real Edge runs of mocked built UI cover six accounts/settings scenarios at
390/900/1440; computed styles and all six screenshot pairs match exactly. Main
visually checked narrow accounts and wide settings. All 30 owners parse alone.
Closing source/checker 19/19/ratchet/encoding and staged/unstaged Git checks pass.
Inputs remained frozen; gates were serialized. Strict remains red for other debt.

Initial process inspection found no Cargo/rustc/npm/Vite/Tauri/gateway process.
Closing observation saw three Cargo/rustc processes with no Gateway command-line
indicator; cwd was not established and processes were preserved. Closing ran no
native/frontend build. The historical pilot avoidance of CSS applied while S06
Cargo gates were active; it is not an exclusive CSS reservation. The coordinator
retains this independent frontend scope. S06's original Rust/Gemini source and
final native build/release transfer remain reserved under GWP-20260912-01.

The boundary scout's proposed split at original line 4,601 bisected a dialog
rule; direct inspection rejected it and used original line 4,582. Raw-source test
readers in four Python suites, including the product contract, also required
composition-aware reads; the initial scout listed only the TypeScript reader.
The first candidate TS reader omitted multiline mode and failed before test
execution. Only its regex g-to-gm correction was needed; retry1 passed. Failed
receipts remain. Two independent read-only reviews found no introduced defect.

Evidence root: target/effective-line-evidence/20260915-desktop-style-owners/.
Scope SHA-256: d2c23bd4db0f3bc837bc29a4d4399cc74a6e983e426506933b085635e5ea93fe.
Inputs 1200; unchanged neighbors 1163. Strict 2073/12/22/40; 34 above 700;
clearance 111/145 (76.6%). Structural scope accepted. Native/live-provider/full
UI/Docker/release and full optimization remain open. No official formatter is
configured for this scope; existing parser/typecheck/encoding/diff checks passed.
[Detailed acceptance](../../status/2026-09-15-desktop-style-owners.md).

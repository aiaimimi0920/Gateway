# Lane I: desktop source-contract decomposition

Coordinator owns the four desktop contract files below. No production UI/CSS,
Rust, release scripts, dependency or policy change belongs to this extraction.

The assigned implementation agent returned without edits after shell quoting
problems. Coordinator captured the exact current dirty source, not HEAD, to
target/effective-line-evidence/desktop-contract-split/before.py and personally
completed the move. Existing refill assertions and the Cargo-environment test
were preserved along with all other prior edits.

| File under tests/python/ | Effective lines | Test methods | Responsibility |
| --- | ---: | ---: | --- |
| test_gateway_desktop_ui_contract.py |205 |9 | Foundation, theme, account/refill wiring |
| test_gateway_desktop_release_contract.py |167 |13 | Build, packaging and smoke script contracts |
| test_gateway_desktop_runtime_contract.py |219 |11 | Runtime/process/profile validation |
| test_gateway_desktop_workbench_contract.py |261 |13 | Onboarding, derived state, diagnostics and panels |

Original830 effective lines. All46 original/current/unique method source segments
match exactly under Python AST, including strings and assertions. Original entry
retains real tests; no forwarding compatibility wrapper or shared fixture layer.
All siblings retain the same directory-based repository root resolution.
CI uses test_*.py discovery, so siblings are included without changing CI.

## Gate evidence and honest limitations

- Exact pre/post matrices each46 tests,45 passes and the SAME theme-selector
  failure: test_desktop_uses_neuroterminal_theme_tokens expects .nt-kicker.
  The assertion was not deleted or weakened to hide a pre-existing failure.
- Broad test_gateway_desktop_*_contract.py discovery also includes six existing
  shell-contract tests and reports52 tests/two failures. Its additional mobile
  shell-layout failure is not created by this split and remains unresolved.
- Checker19/19, ratchet, Python parse/import and Gateway diff checks pass.
- All four files are UTF8 without BOM/NUL. No test method or assertion was lost.
- Evidence:before.log, after.log (broader52), after-exact.log (paired46),
  checker.log, ratchet.log and strict.log in the lane evidence directory.

This clears a structural debt item but is NOT a green desktop/UI acceptance gate.
Next: inspect actual canonical theme ownership and mobile-shell contract before
any separate correction. No binary release is fabricated for a pure test move;
include it in the next agreed integrated build snapshot.

## Separate stale-contract correction

After preserving the red baseline at the pure-move checkpoint, coordinator traced
both failures to stale source ownership rather than changing production to satisfy
obsolete strings. No production source uses nt-kicker or nt-brand__copy. App.tsx
now delegates the shell to features/shell/AppShell.tsx, which renders nt-brand__name.
The corresponding CSS has min-width/overflow/ellipsis safeguards. The final720px
media override uses horizontally scrollable flex navigation, not the earlier grid.

Theme contract now asserts the actual brand class in both CSS and its owning TSX.
Product contract verifies App-to-AppShell wiring, the scoped brand declarations
and the final narrow-screen nav override. It no longer passes merely because a
superseded three-column declaration exists somewhere in the stylesheet.
This deliberately changes two source-contract assertions after the exact-move
proof; it does not claim all46 bodies are unchanged after the correction.

Fresh desktop discovery52/52 passes (the46 split methods plus6 product contracts),
ratchet and diff pass. Updated UI contract207 effective, product contract88;
both UTF8 without BOM/NUL. Evidence:corrected.log and corrected-ratchet.log.
No CSS, TSX, palette, layout or runtime changed. Source contracts do not prove
rendered mobile usability or visual acceptance; those remain final UI gate work.

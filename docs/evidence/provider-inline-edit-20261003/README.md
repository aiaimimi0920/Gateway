# Credential card inline edit verification

Base: `b3111083e2d6f32f7fca4c2582d8237a423b276a` (main after PR #18).
The user-supplied reference image was materialized locally, verified readable,
and visually inspected before implementation. These captures use synthetic
credentials, local path strings and `example.invalid` URLs in the real React app.

The existing two-column card structure is retained. Path, password and numeric
editors stay on one row. Refill notification and inquiry display their real values
with CSS ellipsis; copying still passes the original complete value directly to
the clipboard. Password inputs remain masked and cancelled drafts are cleared.
At narrow widths only editing labels yield space to readable inputs; full labels
remain in accessible names and hover text. Numeric validation appears above the
editor without shifting rows or intercepting endpoint-copy controls.

| Capture | Before | After |
| --- | --- | --- |
| Desktop storage editor | [Before](desktop-before.png) | [After](desktop-after.png) |
| Pixel 7 storage editor | [Before](mobile-before.png) | [After](mobile-after.png) |

[320px numeric editor](narrow-number.png) shows both digits of `99` and the native
spinner, save and cancel controls. [Validation error](validation-error.png) stays
above the editing row and leaves the notification copy control unobstructed.

[DOM measurements](geometry-summary.json) cover five fields at the default,
320px and 1024px viewports in Desktop Chrome and Pixel 7 projects. Before the fix,
desktop rows grew 40→49px and the card 500→506px; mobile rows grew 40→47px even
when the card's existing height floor concealed the overall height change.
After the fix, all 30 editing cases retain 40px rows, a 500px card, and exactly
equal row/card/adjacent-card geometry before, during and after editing.

Validation completed locally:

- Full frontend: 89 files, 446 tests passed. After the final label-title change,
  dependent card/controller/style tests passed again: 3 files, 23 tests.
- TypeScript check and production web build passed.
- Chromium: 10/10 passed, including six inline-editor cases, desktop/mobile flip
  and focus checks, and tablet scrolling. Checks include complete long-URL copies,
  long Windows/Unix paths, input content width, child bounds, Enter/Escape,
  save/cancel, password masking/clearing and error-state copy-button hit testing.
- Effective-line checker tests 31/31, ratchet, security contracts 17/17 and
  whitespace checks passed. No standalone frontend lint/formatter script exists.
- Independent read-only source and screenshot review found no blocking issue.

The broader pre-existing LongCat preview test fails on both main and this branch
at the same `7/30` expectation; this was reproduced against an unmodified main
worktree. The targeted existing card CI selects the flip test, and now also runs
the new inline-editor suite. No existing test is skipped or weakened to hide this
unrelated demo-fixture failure.

No dependency, backend, credential or security-gate changes are included. PR #17
and the independent three-file security PR #19 remain untouched. The only workflow
change adds these UI regressions to the existing card-check command. Remote
exact-head CI and real vulnerability gates remain required before merge; a main
merge's packaging/publication effects require separate authorization.

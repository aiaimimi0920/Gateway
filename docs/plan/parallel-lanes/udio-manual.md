# Lane E: Udio manual browser helper

Owner: coordinator personally. State: released (interim).
Date: 2026-09-08.

## Scope and measured result

| File | Effective lines | Physical lines |
| --- | ---: | ---: |
| scripts/udio-manual-browser-helper.mjs | 797 -> 373 | 878 -> 413 |
| scripts/udio-manual-browser/settings.mjs | 27 | 30 |
| scripts/udio-manual-browser/profile.mjs | 161 | 178 |
| scripts/udio-manual-browser/persistence.mjs | 116 | 128 |
| scripts/udio-manual-browser/snapshot.mjs | 139 | 150 |
| scripts/tests/udio-manual-browser.test.mjs | 148 | 160 |

Settings retain parsing/default rules; profile owns discovery and narrow profile
copy operations; persistence owns JSON/object I/O and the existing singleton S3
client; snapshot owns auth detection and change-deduplicated storage export.
The entry retains browser/context lifetime, lock handling, capture registration,
signals, polling and cleanup. No dependency, lockfile, baseline or exception
change was needed. Existing native MJS runtime is preserved.

## Evidence

- Before extraction: seven synthetic offline tests passed using a VM containing
  declarations only, never executing the browser entry point.
- After extraction: the same seven assertions passed against imported modules.
  Added local-object persistence and actual-entry lock-reuse tests bring the
  focused suite to nine passing tests.
- TypeScript AST extraction compared all 26 original function bodies, including
  main and nested browser/capture callbacks: exact text equality, zero mismatch.
- All six affected MJS files passed Node syntax checks. Production modules and
  entry are UTF-8 without BOM; scoped git diff --check passed.
- Official checker tests passed 19/19 and adoption ratchet passed. Strict remains exit 1:
  1014 scanned, 46 hard, 77 mandatory, 38 soft; 123 files still exceed 700.
  This clears one additional original debt item, not the complete S11 milestone.
- Independent default-role read-only reviewer reran 9/9 focused tests, inspected
  imports, singleton/global lifetime and confirmed recursive package inclusion
  in tools/package-gateway-release.ps1. No confirmed extraction regression.
- Reviewer raised whether new files need baseline entries: they do not.
  The official ratchet passes without changing the adoption baseline; regenerating
  that baseline would violate this task's governance boundary.
- Final independent repository checks: Gateway and Neuro root git diff --check
  both exit 0. Gateway has 301 dirty status entries and Neuro root has 39; these
  are the shared checkout's existing combined work, not this lane's change count.
  No sibling repository was modified, validated as a product, reset or committed.

Commands from Gateway:

    node --test scripts/tests/udio-manual-browser.test.mjs
    node --check <each of the six affected MJS files>
    npm run test:effective-lines --prefix scripts
    npm run check:effective-lines --prefix scripts
    npm run strict:effective-lines --prefix scripts
    git diff --check

Evidence directory: target/effective-line-evidence/20260908-udio-manual/.
The original source snapshot is before.mjs; focused/checker/ratchet/strict logs
remain local ignored evidence, not runtime/package inputs.

## Safety review and limits

Synthetic temporary directories only; no real profile read, browser launch,
provider call, remote S3 write, deployment, Docker or shared Cargo operation.
The actual-entry integration test supplies the current test PID in a temporary
lock and an explicit existing executable, so it returns before profile access.
Executable discovery still precedes lock reuse, exactly as before.

Existing non-atomic lock acquisition, plaintext credential-bearing status files,
copy recursion behavior and cleanup gaps before the main try/finally are not
silently changed or certified safe. Cleanup after entering the existing try block
retains final export -> browser close -> cloned profile removal -> lock removal.
The singleton S3 configuration behavior, global status fields and caller-owned
hash refs are unchanged. Remote S3/browser success and full lifecycle failure
injection are not covered by this offline suite.

## Release boundary

GWP-20260908-03 received explicit source/document freeze and build transfer.
New immutable version20260908-udio-s06-034608 contains lane E and the integrated
S06 checkpoint. Official build/package, 134 Node tests, 14 Python package tests,
UI integrity and two independent10-check runtime smokes passed.
See [release evidence](../../status/2026-09-08-udio-s06-release.md).
The preceding20260908-parallel-refactor-002354 package remains unchanged.
